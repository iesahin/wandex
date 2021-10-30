use thiserror::Error;


use termion::event::Key;
use parking_lot::Mutex;

use std::path::PathBuf;
use std::sync::Arc;

use crate::foldview::LogEntry;
use crate::mediaview::MediaError;




#[derive(Error, Debug, Clone)]
pub enum WError {
    #[error("IO error: {} ", _0)]
    IoError(String),
    #[error("Mutex failed")]
    MutexError,
    #[error("Can't lock!")]
    TryLockError,
    #[error("Channel failed: {}", error)]
    ChannelTryRecvError{error: std::sync::mpsc::TryRecvError},
    #[error("Channel failed: {}", error)]
    ChannelRecvError{error: std::sync::mpsc::RecvError},
    #[error("Channel failed")]
    ChannelSendError,
    #[error("Timer ran out while waiting for message on channel!")]
    ChannelRecvTimeout(std::sync::mpsc::RecvTimeoutError),
    #[error("Previewer failed on file: {}", file)]
    PreviewFailed{file: String},
    #[error("StalePreviewer for file: {}", file)]
    StalePreviewError{file: String},
    #[error("Accessed stale value")]
    StaleError,
    #[error("Failed: {}", _0)]
    Error(String),
    #[error("Was None!")]
    NoneError,
    #[error("Async Error: {}", _0)]
    AError(async_value::AError),
    #[error("No widget found")]
    NoWidgetError,
    #[error("Path: {:?} not in this directory: {:?}", path, dir)]
    WrongDirectoryError{ path: PathBuf, dir: PathBuf},
    #[error("Widget finnished")]
    PopupFinished,
    #[error("No completions found")]
    NoCompletionsError,
    #[error("No more history")]
    NoHistoryError,
    #[error("No core for widget")]
    NoWidgetCoreError,
    #[error("No header for widget")]
    NoHeaderError,
    #[error("You wanted this!")]
    Quit,
    #[error("HBox ratio mismatch: {} widgets, ratio is {:?}", wnum, ratio)]
    HBoxWrongRatioError{ wnum: usize, ratio: Vec<usize> },
    #[error("Got wrong widget: {}! Wanted: {}", got, wanted)]
    WrongWidgetError{got: String, wanted: String},
    #[error("Strip Prefix Error: {}", error)]
    StripPrefixError{error: std::path::StripPrefixError},
    #[error("INofify failed: {}", _0)]
    INotifyError(String),
    #[error("Tags not loaded yet")]
    TagsNotLoadedYetError,
    #[error("Undefined key: {:?}", key)]
    WidgetUndefinedKeyError{key: Key},
    #[error("Terminal has been resized!")]
    TerminalResizedError,
    #[error("Widget has been resized!")]
    WidgetResizedError,
    #[error("{}", _0)]
    Log(String),
    #[error("Metadata already processed")]
    MetadataProcessedError,
    #[error("No files to take from widget")]
    WidgetNoFilesError,
    #[error("Invalid line in settings file: {}", _0)]
    ConfigLineError(String),
    #[error("New input in Minibuffer")]
    MiniBufferInputUpdated(String),
    #[error("Failed to parse into UTF8")]
    UTF8ParseError(std::str::Utf8Error),
    #[error("Failed to parse integer!")]
    ParseIntError(std::num::ParseIntError),
    #[error("Failed to parse char!")]
    ParseCharError(std::char::ParseCharError),
    #[error("{}", _0)]
    Media(MediaError),
    #[error("{}", _0)]
    Mime(MimeError),
    #[error("{}", _0)]
    KeyBind(KeyBindError),
    #[error("FileBrowser needs to know about all tab's files to run exec!")]
    FileBrowserNeedTabFiles,
    #[error("{}", _0)]
    FileError(crate::files::FileError),
    #[error("{}", _0)]
    Nix(nix::Error),
    #[error("Refresh parent widget!")]
    RefreshParent,
    #[error("Refresh parent widget!")]
    MiniBufferEvent(crate::minibuffer::MiniBufferEvent),
    #[error("Bookmark not found!")]
    BookmarkNotFound,
    #[error("No bookmark path found!")]
    BookmarkPathNotFound,
}

// impl Error for HError {}

impl WError {
    pub fn log<T>(log: &str) -> WResult<T> {
        Err(WError::Log(String::from(log))).log_and()
    }
    pub fn quit() -> WResult<()> {
        Err(WError::Quit)
    }
    pub fn wrong_ratio<T>(wnum: usize, ratio: Vec<usize>) -> WResult<T> {
        Err(WError::HBoxWrongRatioError{ wnum, ratio })
    }
    pub fn no_widget<T>() -> WResult<T> {
        Err(WError::NoWidgetError)
    }
    pub fn wrong_widget<T>(got: &str, wanted: &str) -> WResult<T> {
        Err(WError::WrongWidgetError{ got: got.to_string(),
                                      wanted: wanted.to_string() })

    }
    pub fn popup_finished<T>() -> WResult<T> {
        Err(WError::PopupFinished)
    }
    pub fn tags_not_loaded<T>() -> WResult<T> {
        Err(WError::TagsNotLoadedYetError)
    }
    pub fn undefined_key<T>(key: Key) -> WResult<T> {
        Err(WError::WidgetUndefinedKeyError { key })
    }
    pub fn wrong_directory<T>(path: PathBuf, dir: PathBuf) -> WResult<T> {
        Err(WError::WrongDirectoryError{ path,
                                         dir })

    }
    pub fn preview_failed<T>(file: &crate::files::File) -> WResult<T> {
        let name = file.name.clone();
        Err(WError::PreviewFailed{ file: name })

    }

    pub fn terminal_resized<T>() -> WResult<T> {
        Err(WError::TerminalResizedError)
    }

    pub fn widget_resized<T>() -> WResult<T> {
        Err(WError::WidgetResizedError)
    }

    pub fn stale<T>() -> WResult<T> {
        Err(WError::StaleError)
    }

    pub fn config_error<T>(line: String) -> WResult<T> {
        Err(WError::ConfigLineError(line))
    }

    pub fn metadata_processed<T>() -> WResult<T> {
        Err(WError::MetadataProcessedError)
    }

    pub fn no_files<T>() -> WResult<T> {
        Err(WError::WidgetNoFilesError)
    }

    pub fn input_updated<T>(input: String) -> WResult<T> {
        Err(WError::MiniBufferInputUpdated(input))
    }


}

#[derive(Error, Debug, Clone)]
pub enum ErrorCause {
    #[error("{}", _0)]
    Str(String)
}


lazy_static! {
    static ref LOG: Mutex<Vec<LogEntry>> = Mutex::new(vec![]);
}

pub fn get_logs() -> WResult<Vec<LogEntry>> {
    let logs = LOG.lock().drain(..).collect();
    Ok(logs)
}

pub fn put_log<L: Into<LogEntry>>(log: L) -> WResult<()> {
    LOG.lock().push(log.into());
    Ok(())
}

pub trait ErrorLog where Self: Sized {
    fn log(self);
    fn log_and(self) -> Self;
}

// impl<T> ErrorLog for HResult<T> {
//     fn log(self) {
//         if let Err(err) = self {
//             put_log(&err).ok();
//         }
//     }

//     fn log_and(self) -> Self {
//         if let Err(err) = &self {
//             put_log(err).ok();
//         }
//         self
//     }
// }


// impl<T> ErrorLog for Result<T, AError> {
//     fn log(self) {
//         if let Err(err) = self {
//             put_log(&err.into()).ok();
//         }
//     }

//     fn log_and(self) -> Self {
//         if let Err(err) = &self {
//             put_log(&err.clone().into()).ok();
//         }
//         self
//     }
// }

impl<T, E> ErrorLog for Result<T, E>
where E: Into<WError> + Clone {
    fn log(self) {
        if let Err(err) = self {
            let err: WError = err.into();
            put_log(&err).ok();
        }
    }
    fn log_and(self) -> Self {
        if let Err(ref err) = self {
            let err: WError = err.clone().into();
            put_log(&err).ok();
        }
        self
    }
}

impl<E> ErrorLog for E
where E: Into<WError> + Clone {
    fn log(self) {
        let err: WError = self.into();
        put_log(&err).ok();

    }
    fn log_and(self) -> Self {
        let err: WError = self.clone().into();
        put_log(&err).ok();
        self
    }
}

impl From<std::io::Error> for WError {
    fn from(error: std::io::Error) -> Self {
        let err = WError::IoError(format!("{}", error));
        err
    }
}

// impl From<failure::Error> for WError {
//     fn from(error: failure::Error) -> Self {
//         let err = WError::Error(format!("{}", error));
//         err
//     }
// }
//
impl From<std::sync::mpsc::TryRecvError> for WError {
    fn from(error: std::sync::mpsc::TryRecvError) -> Self {
        let err = WError::ChannelTryRecvError { error };
        err
    }
}

impl From<std::sync::mpsc::RecvError> for WError {
    fn from(error: std::sync::mpsc::RecvError) -> Self {
        let err = WError::ChannelRecvError { error };
        err
    }
}

impl From<std::sync::mpsc::RecvTimeoutError> for WError {
    fn from(error: std::sync::mpsc::RecvTimeoutError) -> Self {
        let err = WError::ChannelRecvTimeout(error);
        err
    }
}

impl<T> From<std::sync::mpsc::SendError<T>> for WError {
    fn from(_error: std::sync::mpsc::SendError<T>) -> Self {
        let err = WError::ChannelSendError;
        err
    }
}

impl<T> From<std::sync::PoisonError<T>> for WError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        let err = WError::MutexError;
        err
    }
}

impl<T> From<std::sync::TryLockError<T>> for WError {
    fn from(_error: std::sync::TryLockError<T>) -> Self {
        let err = WError::TryLockError;
        err
    }
}

// The following requires nightly
// impl From<std::option::NoneError> for HError {
//     fn from(_error: std::option::NoneError) -> Self {
//         let err = HError::NoneError;
//         err
//     }
// }

impl From<std::path::StripPrefixError> for WError {
    fn from(error: std::path::StripPrefixError) -> Self {
        let err = WError::StripPrefixError{ error };
        err
    }
}

impl From<notify::Error> for WError {
    fn from(error: notify::Error) -> Self {
        let err = WError::INotifyError(format!("{}", error));
        err
    }
}

impl From<async_value::AError> for WError {
    fn from(error: async_value::AError) -> Self {
        let err = WError::AError(error);
        err
    }
}

impl From<std::str::Utf8Error> for WError {
    fn from(error: std::str::Utf8Error) -> Self {
        let err = WError::UTF8ParseError(error);
        err
    }
}


impl From<std::num::ParseIntError> for WError {
    fn from(error: std::num::ParseIntError) -> Self {
        let err = WError::ParseIntError(error);
        err
    }
}

impl From<nix::Error> for WError {
    fn from(error: nix::Error) -> Self {
        let err = WError::Nix(error);
        err
    }
}

impl From<std::char::ParseCharError> for WError {
    fn from(error: std::char::ParseCharError) -> Self {
        let err = WError::ParseCharError(error);
        err
    }
}


// MIME Errors

#[derive(Error, Debug, Clone)]
pub enum MimeError {
    #[error("Need a file to determine MIME type")]
    NoFileProvided,
    #[error("File access failed! Error: {}", _0)]
    AccessFailed(Box<WError>),
    #[error("No MIME type found for this file",)]
    NoMimeFound,
    #[error("Panicked while trying to find MIME type for: {}!", _0)]
    Panic(String),
}

impl From<MimeError> for WError {
    fn from(e: MimeError) -> Self {
        WError::Mime(e)
    }
}


impl From<KeyBindError> for WError {
    fn from(e: KeyBindError) -> Self {
        WError::KeyBind(e)
    }
}

impl From<crate::minibuffer::MiniBufferEvent> for WError {
    fn from(e: crate::minibuffer::MiniBufferEvent) -> Self {
        WError::MiniBufferEvent(e)
    }
}

#[derive(Error, Debug, Clone)]
pub enum KeyBindError {
    #[error("Movement has not been defined for this widget")]
    MovementUndefined,
    #[error("Keybind defined with wrong key: {} -> {}", _0, _1)]
    WrongKey(String, String),
    #[error("Defined keybind for non-existing action: {}", _0)]
    WrongAction(String),
    #[error("Failed to parse keybind: {}", _0)]
    ParseKeyError(String),
    #[error("Trouble with ini file! Error: {}", _0)]
    IniError(Arc<ini::Error>),
    #[error("Couldn't parse as either char or u8: {}", _0)]
    CharOrNumParseError(String),
    #[error("Wanted {}, but got {}!", _0, _1)]
    CharOrNumWrongType(String, String)

}

impl From<ini::Error> for KeyBindError {
    fn from(err: ini::Error) -> Self {
        KeyBindError::IniError(Arc::new(err))
    }
}

impl From<crate::files::FileError> for WError {
    fn from(err: crate::files::FileError) -> Self {
        WError::FileError(err)
    }
}

pub type WResult<T> = Result<T, WError>;