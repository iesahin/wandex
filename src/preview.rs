use async_value::{Async, Stale};
use parking_lot::Mutex;
use termion::event::Key;

use std::path::PathBuf;
use std::sync::Arc;

use crate::coordinates::Coordinates;
use crate::fail::{ErrorLog, WError, WResult};
use crate::files::{File, Files, Kind, Ticker};
use crate::fscache::FsCache;
use crate::imgview::ImgView;
use crate::listview::{FileSource, ListView};
use crate::mediaview::MediaView;
use crate::textview::TextView;
use crate::widget::{Widget, WidgetCore};

pub type AsyncWidgetFn<W> = dyn FnOnce(&Stale, WidgetCore) -> WResult<W> + Send + Sync;

lazy_static! {
    static ref SUBPROC: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
}

fn kill_proc() -> WResult<()> {
    // Kill media previewer if it still runs
    ImgView::kill_running();

    let mut pid = SUBPROC.lock();
    pid.map(|pid|
            // Do this in another thread so we can wait on process to exit with SIGHUP
            std::thread::spawn(move || {
                use nix::{unistd::Pid,
                          sys::signal::{killpg, Signal}};

                let sleep_time = std::time::Duration::from_millis(50);

                // Kill using process group, to clean up all child processes, too
                let pid = Pid::from_raw(pid as i32);
                killpg(pid, Signal::SIGTERM).ok();
                std::thread::sleep(sleep_time);
                killpg(pid, Signal::SIGKILL).ok();
            }));
    *pid = None;
    Ok(())
}

impl<W: Widget + Send + 'static> PartialEq for AsyncWidget<W> {
    fn eq(&self, other: &AsyncWidget<W>) -> bool {
        if self.get_coordinates().unwrap() == other.get_coordinates().unwrap() {
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct AsyncWidget<W: Widget + Send + 'static> {
    pub widget: Async<W>,
    core: WidgetCore,
}

impl<W: Widget + Send + 'static> AsyncWidget<W> {
    pub fn new(
        core: &WidgetCore,
        closure: impl FnOnce(&Stale) -> WResult<W> + Send + 'static,
    ) -> AsyncWidget<W> {
        let sender = Arc::new(Mutex::new(core.get_sender()));
        let mut widget = Async::new(move |stale| closure(stale).map_err(|e| e.into()));
        widget
            .on_ready(move |_, stale| {
                if !stale.is_stale()? {
                    sender
                        .lock()
                        .send(crate::widget::Events::WidgetReady)
                        .map_err(WError::from)
                        .log();
                }
                Ok(())
            })
            .log();

        widget.run().log();

        AsyncWidget {
            widget: widget,
            core: core.clone(),
        }
    }
    pub fn change_to(
        &mut self,
        closure: impl FnOnce(&Stale, WidgetCore) -> WResult<W> + Send + 'static,
    ) -> WResult<()> {
        self.set_stale().log();

        let sender = Mutex::new(self.get_core()?.get_sender());
        let core = self.get_core()?.clone();

        let mut widget = Async::new(move |stale| Ok(closure(stale, core.clone())?));

        widget
            .on_ready(move |_, stale| {
                if !stale.is_stale()? {
                    sender
                        .lock()
                        .send(crate::widget::Events::WidgetReady)
                        .map_err(WError::from)
                        .log();
                }
                Ok(())
            })
            .log();

        widget.run().log();

        self.widget = widget;
        Ok(())
    }

    pub fn set_stale(&mut self) -> WResult<()> {
        Ok(self.widget.set_stale()?)
    }

    pub fn is_stale(&self) -> WResult<bool> {
        Ok(self.widget.is_stale()?)
    }

    pub fn get_stale(&self) -> Stale {
        self.widget.get_stale()
    }

    pub fn widget(&self) -> WResult<&W> {
        Ok(self.widget.get()?)
    }

    pub fn widget_mut(&mut self) -> WResult<&mut W> {
        Ok(self.widget.get_mut()?)
    }

    pub fn take_widget(self) -> WResult<W> {
        Ok(self.widget.value?)
    }

    pub fn ready(&self) -> bool {
        self.widget().is_ok()
    }
}

impl<T: Widget + Send + 'static> Widget for AsyncWidget<T> {
    fn get_core(&self) -> WResult<&WidgetCore> {
        Ok(&self.core)
    }
    fn get_core_mut(&mut self) -> WResult<&mut WidgetCore> {
        Ok(&mut self.core)
    }

    fn set_coordinates(&mut self, coordinates: &Coordinates) -> WResult<()> {
        self.core.coordinates = coordinates.clone();
        if let Ok(widget) = self.widget_mut() {
            widget.set_coordinates(&coordinates)?;
        }
        Ok(())
    }

    fn refresh(&mut self) -> WResult<()> {
        self.widget.pull_async().ok();

        let coords = self.get_coordinates()?.clone();
        if let Ok(widget) = self.widget_mut() {
            if widget.get_coordinates()? != &coords {
                widget.set_coordinates(&coords)?;
                widget.refresh()?;
            } else {
                widget.refresh()?;
            }
        }
        Ok(())
    }
    fn get_drawlist(&self) -> WResult<String> {
        if self.widget().is_err() {
            let clear = self.core.get_clearlist()?;
            let (xpos, ypos) = self.get_coordinates()?.u16position();
            let pos = crate::term::goto_xy(xpos, ypos);
            return Ok(clear + &pos + crate::files::tick_str());
        }

        if self.is_stale()? {
            return self.core.get_clearlist();
        }

        self.widget()?.get_drawlist()
    }
    fn on_key(&mut self, key: termion::event::Key) -> WResult<()> {
        if self.widget().is_err() {
            return Ok(());
        }
        self.widget_mut()?.on_key(key)
    }
    fn render_footer(&self) -> WResult<String> {
        if self.widget().is_err() {
            return Ok(String::new());
        }
        self.widget()?.render_footer()
    }
}

impl PartialEq for Previewer {
    fn eq(&self, other: &Previewer) -> bool {
        if self.widget.get_coordinates().unwrap() == other.widget.get_coordinates().unwrap() {
            true
        } else {
            false
        }
    }
}

#[derive(PartialEq)]
enum PreviewWidget {
    FileList(ListView<Files>),
    TextView(TextView),
    ImgView(ImgView),
    MediaView(MediaView),
}

enum ExtPreviewer {
    Text(PathBuf),
    Graphics(PathBuf),
}

fn find_previewer(file: &File, g_mode: bool) -> WResult<ExtPreviewer> {
    let path = crate::paths::previewers_path()?;
    let ext = file.path.extension().ok_or(WError::NoneError)?;

    // Try to find a graphical previewer first
    if g_mode {
        let g_previewer = path
            .read_dir()?
            .find(|previewer| {
                previewer
                    .as_ref()
                    .and_then(|p| {
                        Ok(p.path().file_stem() == Some(ext)
                            && p.path().extension() == Some(&std::ffi::OsStr::new("g")))
                    })
                    .unwrap_or(false)
            })
            .map(|p| p.map(|p| p.path()));
        match g_previewer {
            Some(Ok(g_p)) => return Ok(ExtPreviewer::Graphics(g_p)),
            _ => {}
        }
    }

    // Look for previewers matching the file extension
    let previewer = path
        .read_dir()?
        .find(|previewer| {
            previewer
                .as_ref()
                .and_then(|p| Ok(p.file_name() == ext))
                .unwrap_or(false)
        })
        .map(|p| p.map(|p| p.path()));
    match previewer {
        Some(Ok(p)) => return Ok(ExtPreviewer::Text(p)),
        _ => {
            // Special case to highlight text files that aren't text/*
            if file.is_text() {
                let mut previewer = PathBuf::from(&path);
                previewer.push("definitions/");
                previewer.push("text");
                return Ok(ExtPreviewer::Text(previewer));
            }
        }
    }

    Ok(ExtPreviewer::Text(previewer.ok_or(WError::NoneError)??))
}

pub struct Previewer {
    widget: AsyncWidget<PreviewWidget>,
    core: WidgetCore,
    file: Option<File>,
    pub cache: FsCache,
    animator: Stale,
}

impl Previewer {
    pub fn new(core: &WidgetCore, cache: FsCache) -> Previewer {
        let core_ = core.clone();
        let widget = AsyncWidget::new(&core, move |_| {
            let blank = TextView::new_blank(&core_);
            let blank = PreviewWidget::TextView(blank);
            Ok(blank)
        });

        Previewer {
            widget: widget,
            core: core.clone(),
            file: None,
            cache: cache,
            animator: Stale::new(),
        }
    }

    fn become_preview(&mut self, widget: WResult<AsyncWidget<PreviewWidget>>) -> WResult<()> {
        let coordinates = self.get_coordinates()?.clone();
        self.widget = widget?;
        self.widget.set_coordinates(&coordinates)?;
        Ok(())
    }

    pub fn set_stale(&mut self) -> WResult<()> {
        self.cancel_animation()?;
        self.widget.set_stale()
    }

    pub fn get_file(&self) -> Option<&File> {
        self.file.as_ref()
    }

    pub fn cancel_animation(&self) -> WResult<()> {
        Ok(self.animator.set_stale()?)
    }

    pub fn take_files(&mut self) -> WResult<Files> {
        match self.widget.widget_mut() {
            Ok(PreviewWidget::FileList(file_list)) => {
                let files = std::mem::take(&mut file_list.content);
                Ok(files)
            }
            _ => WError::no_files()?,
        }
    }

    pub fn put_preview_files(&mut self, files: Files, selected_file: Option<File>) {
        let dir = files.directory.clone();
        let cache = self.cache.clone();
        self.file = Some(dir);

        self.widget
            .change_to(move |stale, core| {
                let source = crate::listview::FileSource::Files(files);

                let list = ListView::builder(core.clone(), source)
                    // .prerender()
                    .with_cache(cache)
                    .with_stale(stale.clone())
                    .select(selected_file)
                    .build()?;

                Ok(PreviewWidget::FileList(list))
            })
            .log();
    }

    pub fn set_file(&mut self, file: &File) -> WResult<()> {
        if Some(file) == self.file.as_ref() && !self.widget.is_stale()? {
            return Ok(());
        }
        self.widget.set_stale().ok();

        let same_dir = self
            .file
            .as_ref()
            .map(|f| f.path.parent() == file.path.parent())
            .unwrap_or(true);
        self.file = Some(file.clone());

        let coordinates = self.get_coordinates().unwrap().clone();
        let file = file.clone();
        let core = self.core.clone();
        let cache = self.cache.clone();
        let animator = self.animator.clone();

        if same_dir {
            self.animator.set_fresh().ok();
        } else {
            self.animator.set_stale().ok();
        }

        self.become_preview(Ok(AsyncWidget::new(&self.core, move |stale: &Stale| {
            kill_proc().log();
            // Delete files left by graphical PDF previews, etc.
            if std::path::Path::new("/tmp/hunter-previews").exists() {
                std::fs::remove_dir_all("/tmp/hunter-previews/")
                    .map_err(WError::from)
                    .log();
            }

            if file.kind == Kind::Directory {
                let preview = Previewer::preview_dir(&file, cache, &core, &stale, &animator);
                return Ok(preview?);
            }

            if let Some(mime) = file.get_mime().log_and().ok() {
                let mime_type = mime.type_().as_str();
                let is_gif = mime.subtype() == "gif";
                let has_media = core.config().media_available();

                match mime_type {
                    _ if mime_type == "video" || is_gif && has_media => {
                        let media_type = crate::mediaview::MediaType::Video;
                        let mediaview =
                            MediaView::new_from_file(core.clone(), &file.path, media_type)?;
                        return Ok(PreviewWidget::MediaView(mediaview));
                    }
                    "image" if has_media => {
                        // Show animation while image is loading, Drop stops it automatically
                        Ticker::start_ticking(core.get_sender());
                        let imgview = ImgView::new_from_file(core.clone(), &file.path())?;
                        return Ok(PreviewWidget::ImgView(imgview));
                    }
                    "audio" if has_media => {
                        let media_type = crate::mediaview::MediaType::Audio;
                        let mediaview =
                            MediaView::new_from_file(core.clone(), &file.path, media_type)?;
                        return Ok(PreviewWidget::MediaView(mediaview));
                    }
                    "text" if mime.subtype() == "plain" => {
                        return Ok(Previewer::preview_text(&file, &core, &stale, &animator)?);
                    }
                    _ => {
                        let preview = Previewer::preview_external(&file, &core, &stale, &animator);
                        if preview.is_ok() {
                            return Ok(preview?);
                        }
                    }
                }
            }

            let mut blank = TextView::new_blank(&core);
            blank.set_coordinates(&coordinates).log();
            blank.refresh().log();
            blank.animate_slide_up(Some(&animator)).log();
            return Ok(PreviewWidget::TextView(blank));
        })))
    }

    pub fn reload(&mut self) {
        if let Some(file) = self.file.take() {
            self.set_file(&file).log();
        }
    }

    pub fn reload_text(&mut self) {
        match self.widget.widget_mut() {
            Ok(PreviewWidget::TextView(w)) => w.load_full(),
            _ => {}
        }
    }

    fn preview_failed<T>(file: &File) -> WResult<T> {
        WError::preview_failed(file)
    }

    fn preview_dir(
        file: &File,
        cache: FsCache,
        core: &WidgetCore,
        stale: &Stale,
        animator: &Stale,
    ) -> WResult<PreviewWidget> {
        use crate::dirty::Dirtyable;

        if stale.is_stale()? {
            return Previewer::preview_failed(&file);
        }
        let source = FileSource::Path(file.clone());

        let mut file_list = ListView::builder(core.clone(), source)
            .with_cache(cache)
            .with_stale(stale.clone())
            .build()?;

        if stale.is_stale()? {
            return Previewer::preview_failed(&file);
        }

        // Start loading metadata during animation
        file_list.refresh()?;
        file_list.animate_slide_up(Some(animator))?;
        file_list.core.set_clean();

        Ok(PreviewWidget::FileList(file_list))
    }

    fn preview_text(
        file: &File,
        core: &WidgetCore,
        stale: &Stale,
        animator: &Stale,
    ) -> WResult<PreviewWidget> {
        // Show animation while text is loading
        let mut ticker = Ticker::start_ticking(core.get_sender());

        let lines = core.coordinates.ysize() as usize;

        let mut textview = TextView::new_from_file_limit_lines(&core, &file, lines)?;
        if stale.is_stale()? {
            return Previewer::preview_failed(&file);
        }

        textview.set_coordinates(&core.coordinates)?;
        textview.refresh()?;

        if stale.is_stale()? {
            return Previewer::preview_failed(&file);
        }

        // Prevent flicker during slide up
        ticker.stop_ticking();
        textview.animate_slide_up(Some(animator))?;
        Ok(PreviewWidget::TextView(textview))
    }

    fn run_external(cmd: PathBuf, file: &File, stale: &Stale) -> WResult<Vec<String>> {
        use std::os::unix::process::CommandExt;

        let process = unsafe {
            std::process::Command::new(cmd)
                .arg(&file.path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .pre_exec(|| {
                    let pid = std::process::id();
                    // To make killing subprocess possible create new process group
                    libc::setpgid(pid as i32, pid as i32);
                    Ok(())
                })
                .spawn()?
        };

        let pid = process.id();
        {
            let mut pid_ = SUBPROC.lock();
            *pid_ = Some(pid);
        }

        if stale.is_stale()? {
            return Previewer::preview_failed(&file);
        }
        let output = process.wait_with_output()?;
        if stale.is_stale()? {
            return Previewer::preview_failed(&file);
        }

        {
            let mut pid_ = SUBPROC.lock();
            *pid_ = None;
        }

        //let status = output.status.code()?;

        let output = std::str::from_utf8(&output.stdout)?
            .to_string()
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(output)
    }

    fn preview_external(
        file: &File,
        core: &WidgetCore,
        stale: &Stale,
        animator: &Stale,
    ) -> WResult<PreviewWidget> {
        // Show animation while preview is being generated
        let mut ticker = Ticker::start_ticking(core.get_sender());

        let previewer = if core.config().graphics.as_str() != "unicode" {
            find_previewer(&file, true)?
        } else {
            find_previewer(&file, false)?
        };

        match previewer {
            ExtPreviewer::Text(previewer) => {
                if stale.is_stale()? {
                    return Previewer::preview_failed(&file);
                }
                let lines = Previewer::run_external(previewer, file, stale)?;
                if stale.is_stale()? {
                    return Previewer::preview_failed(&file);
                }

                let mut textview = TextView::new_blank(&core);
                textview.set_lines(lines)?;
                textview.set_coordinates(&core.coordinates).log();
                textview.refresh().log();
                // Prevent flicker during slide up
                ticker.stop_ticking();
                textview.animate_slide_up(Some(animator)).log();

                Ok(PreviewWidget::TextView(textview))
            }
            ExtPreviewer::Graphics(previewer) => {
                let lines = Previewer::run_external(previewer, file, stale)?;
                let gfile = lines.first().ok_or(WError::NoneError)?;
                let imgview = ImgView::new_from_file(core.clone(), &PathBuf::from(&gfile))?;
                Ok(PreviewWidget::ImgView(imgview))
            }
        }
    }
}

impl Widget for Previewer {
    fn get_core(&self) -> WResult<&WidgetCore> {
        Ok(&self.core)
    }
    fn get_core_mut(&mut self) -> WResult<&mut WidgetCore> {
        Ok(&mut self.core)
    }

    fn config_loaded(&mut self) -> WResult<()> {
        use PreviewWidget::*;

        let show_hidden = self.core.config().show_hidden();

        match self.widget.widget_mut() {
            Ok(FileList(filelist)) => {
                let setting = filelist.content.show_hidden;

                if setting != show_hidden {
                    self.reload();
                }
            }
            Ok(_) => {}
            Err(_) => self.reload(),
        }

        Ok(())
    }

    fn set_coordinates(&mut self, coordinates: &Coordinates) -> WResult<()> {
        self.core.coordinates = coordinates.clone();
        self.widget.set_coordinates(&coordinates)
    }

    fn refresh(&mut self) -> WResult<()> {
        self.widget.refresh()
    }
    fn get_drawlist(&self) -> WResult<String> {
        self.widget.get_drawlist()
    }

    fn render_footer(&self) -> WResult<String> {
        self.widget.render_footer()
    }

    fn on_key(&mut self, key: Key) -> WResult<()> {
        self.widget.on_key(key)
    }
}

impl Widget for PreviewWidget {
    fn get_core(&self) -> WResult<&WidgetCore> {
        match self {
            PreviewWidget::FileList(widget) => widget.get_core(),
            PreviewWidget::TextView(widget) => widget.get_core(),
            PreviewWidget::ImgView(widget) => widget.get_core(),
            PreviewWidget::MediaView(widget) => widget.get_core(),
        }
    }
    fn get_core_mut(&mut self) -> WResult<&mut WidgetCore> {
        match self {
            PreviewWidget::FileList(widget) => widget.get_core_mut(),
            PreviewWidget::TextView(widget) => widget.get_core_mut(),
            PreviewWidget::ImgView(widget) => widget.get_core_mut(),
            PreviewWidget::MediaView(widget) => widget.get_core_mut(),
        }
    }
    fn set_coordinates(&mut self, coordinates: &Coordinates) -> WResult<()> {
        match self {
            PreviewWidget::FileList(widget) => widget.set_coordinates(coordinates),
            PreviewWidget::TextView(widget) => widget.set_coordinates(coordinates),
            PreviewWidget::ImgView(widget) => widget.set_coordinates(coordinates),
            PreviewWidget::MediaView(widget) => widget.set_coordinates(coordinates),
        }
    }
    fn refresh(&mut self) -> WResult<()> {
        match self {
            PreviewWidget::FileList(widget) => widget.refresh(),
            PreviewWidget::TextView(widget) => widget.refresh(),
            PreviewWidget::ImgView(widget) => widget.refresh(),
            PreviewWidget::MediaView(widget) => widget.refresh(),
        }
    }
    fn get_drawlist(&self) -> WResult<String> {
        match self {
            PreviewWidget::FileList(widget) => widget.get_drawlist(),
            PreviewWidget::TextView(widget) => widget.get_drawlist(),
            PreviewWidget::ImgView(widget) => widget.get_drawlist(),
            PreviewWidget::MediaView(widget) => widget.get_drawlist(),
        }
    }

    fn render_footer(&self) -> WResult<String> {
        match self {
            PreviewWidget::FileList(widget) => widget.render_footer(),
            PreviewWidget::TextView(widget) => widget.render_footer(),
            PreviewWidget::ImgView(widget) => widget.render_footer(),
            PreviewWidget::MediaView(widget) => widget.render_footer(),
        }
    }

    fn on_key(&mut self, key: Key) -> WResult<()> {
        match self {
            PreviewWidget::FileList(widget) => widget.on_key(key),
            PreviewWidget::TextView(widget) => widget.on_key(key),
            PreviewWidget::ImgView(widget) => widget.on_key(key),
            PreviewWidget::MediaView(widget) => widget.on_key(key),
        }
    }
}

impl<T> Widget for Box<T>
where
    T: Widget + ?Sized,
{
    fn get_core(&self) -> WResult<&WidgetCore> {
        Ok((**self).get_core()?)
    }
    fn get_core_mut(&mut self) -> WResult<&mut WidgetCore> {
        Ok((**self).get_core_mut()?)
    }
    fn render_header(&self) -> WResult<String> {
        (**self).render_header()
    }
    fn refresh(&mut self) -> WResult<()> {
        (**self).refresh()
    }
    fn get_drawlist(&self) -> WResult<String> {
        (**self).get_drawlist()
    }
}
