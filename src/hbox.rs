use termion::event::{Event};

use crate::widget::{Widget, WidgetCore};
use crate::coordinates::{Coordinates, Size, Position};
use crate::fail::{WResult, WError, ErrorLog};

#[derive(Debug, PartialEq)]
pub struct HBox<T: Widget> {
    pub core: WidgetCore,
    pub widgets: Vec<T>,
    pub ratios: Option<Vec<usize>>,
    pub zoom_active: bool,
    pub active: Option<usize>,
}


impl<T> HBox<T> where T: Widget + PartialEq {
    pub fn new(core: &WidgetCore) -> HBox<T> {
        HBox { core: core.clone(),
               widgets: vec![],
               ratios: None,
               zoom_active: false,
               active: None
         }
    }


    pub fn resize_children(&mut self) -> WResult<()> {
        let len = self.widgets.len();
        if len == 0 { return Ok(()) }

        if self.zoom_active {
            let coords = self.core.coordinates.clone();
            self.active_widget_mut()?.set_coordinates(&coords).log();
            return Ok(());
        }

        let coords: Vec<Coordinates> = self.calculate_coordinates()?;


        for (widget, coord) in self.widgets.iter_mut().zip(coords.iter()) {
            widget.set_coordinates(coord).log();
        }

        Ok(())
    }

    pub fn push_widget(&mut self, widget: T) {
        self.widgets.push(widget);
    }

    pub fn pop_widget(&mut self) -> Option<T> {
        let widget = self.widgets.pop();
        widget
    }

    pub fn remove_widget(&mut self, index: usize) -> T {
        self.widgets.remove(index)
    }

    pub fn prepend_widget(&mut self, widget: T) {
        self.widgets.insert(0, widget);
    }

    pub fn insert_widget(&mut self, index: usize, widget: T) {
        self.widgets.insert(index, widget);
    }

    pub fn replace_widget(&mut self, index: usize, mut widget: T) -> T {
        std::mem::swap(&mut self.widgets[index], &mut widget);
        widget
    }

    pub fn toggle_zoom(&mut self) -> WResult<()> {
        self.core.clear().log();
        self.zoom_active = !self.zoom_active;
        self.resize_children()
    }

    pub fn set_ratios(&mut self, ratios: Vec<usize>) {
        self.ratios = Some(ratios);
    }

    pub fn calculate_equal_ratios(&self) -> WResult<Vec<usize>> {
        let len = self.widgets.len();
        if len == 0 { return WError::no_widget(); }

        let ratios = (0..len).map(|_| 100 / len).collect();
        Ok(ratios)
    }

    pub fn calculate_coordinates(&self) -> WResult<Vec<Coordinates>> {
        let box_coords = self.get_coordinates()?;
        let box_xsize = box_coords.xsize();
        let box_ysize = box_coords.ysize();
        let box_top = box_coords.top().y();

        let ratios = match self.ratios.clone() {
            Some(ratios) => ratios,
            None => self.calculate_equal_ratios()?
        };

        let ratios_sum: usize = ratios.iter().sum();

        let mut ratios = ratios.iter()
                               .map(|&r| (r as f64 * box_xsize as f64 / ratios_sum as f64).round() as usize)
                               .map(|r| if r < 10 { 10 } else { r })
                               .collect::<Vec<_>>();

        let mut ratios_sum: usize = ratios.iter().sum();

        while ratios_sum + ratios.len() > box_xsize as usize + 1 {
            let ratios_max = ratios.iter()
                                   .position(|&r| r == *ratios.iter().max().unwrap())
                                   .unwrap();
            ratios[ratios_max] = ratios[ratios_max] - 1;
            ratios_sum -= 1;
        }

        let coords = ratios.iter().fold(Vec::<Coordinates>::new(), |mut coords, ratio| {
            let len = coords.len();
            let gap = if len == ratios.len() { 0 } else { 1 };

            let widget_xsize = *ratio as u16;
            let widget_xpos = if len == 0 {
                box_coords.top().x()
            } else {
                let prev_coords = coords.last().unwrap();
                let prev_xsize = prev_coords.xsize();
                let prev_xpos = prev_coords.position().x();

                prev_xsize + prev_xpos + gap
            };

            coords.push(Coordinates {
                size: Size((widget_xsize,
                            box_ysize)),
                position: Position((widget_xpos,
                                    box_top))
            });
            coords
        });

        Ok(coords)
    }

    pub fn set_active(&mut self, i: usize) -> WResult<()> {
        if i+1 > self.widgets.len() {
            WError::no_widget()?
        }
        self.active = Some(i);
        Ok(())
    }

    pub fn active_widget(&self) -> Option<&T> {
        self.widgets.get(self.active?)
    }

    pub fn active_widget_mut(&mut self) -> Option<&mut T> {
        self.widgets.get_mut(self.active?)
    }
}




impl<T> Widget for HBox<T> where T: Widget + PartialEq {
    fn get_core(&self) -> WResult<&WidgetCore> {
        Ok(&self.core)
    }
    fn get_core_mut(&mut self) -> WResult<&mut WidgetCore> {
        Ok(&mut self.core)
    }

    fn set_coordinates(&mut self, coordinates: &Coordinates) -> WResult<()> {
        self.core.coordinates = coordinates.clone();
        self.resize_children()
    }

    fn render_header(&self) -> WResult<String> {
        self.active_widget()?.render_header()
    }

    fn refresh(&mut self) -> WResult<()> {
        if self.zoom_active {
            self.active_widget_mut()?.refresh().log();
            return Ok(());
        }

        self.resize_children().log();
        for child in &mut self.widgets {
            child.refresh().log();
        }
        Ok(())
    }

    fn get_drawlist(&self) -> WResult<String> {
        if self.zoom_active {
            return self.active_widget()?.get_drawlist();
        }

        Ok(self.widgets.iter().map(|child| {
            child.get_drawlist().log_and().unwrap_or_else(|_| String::new())
        }).collect())
    }

    fn on_event(&mut self, event: Event) -> WResult<()> {
        self.active_widget_mut()?.on_event(event)?;
        Ok(())
    }

    fn on_key(&mut self, key: termion::event::Key) -> WResult<()> {
        self.active_widget_mut()?.on_key(key)
    }
}
