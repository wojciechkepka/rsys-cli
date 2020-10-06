use super::{
    common::{single_widget_loop, DataSeries, GraphWidget, Monitor, StatefulWidget},
    events::Config,
};
use crate::util::{conv_fhz, conv_hz, random_color};
use anyhow::{anyhow, Result};
use rsys::linux::cpu::{processor, Core};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::Span,
    widgets::{Dataset, Row, Table},
    Frame,
};

const X_AXIS: (f64, f64) = (0., 30.0);
const Y_AXIS: (f64, f64) = (f64::MAX, 0.);
const TICK_RATE: u64 = 250;

// Stats of a single core
struct CoreStat {
    name: String,
    color: Color,
    data: DataSeries,
    core: Core,
}
impl From<Core> for CoreStat {
    fn from(core: Core) -> Self {
        Self {
            name: format!("cpu{}", core.id),
            color: random_color(Some(20)),
            data: DataSeries::new(),
            core,
        }
    }
}
impl CoreStat {
    // Updates core and returns its new frequency
    fn update(&mut self, time: f64) -> Result<f64> {
        self.core
            .update()
            .map_err(|e| anyhow!("Failed to update core `{}` frequency - {}", self.name, e))?;
        self.add_current(time);
        Ok(self.core.cur_freq as f64)
    }

    fn add_current(&mut self, time: f64) {
        self.data.add(time, self.core.cur_freq as f64);
    }
}

pub struct CpuMonitor {
    stats: Vec<CoreStat>,
    m: Monitor,
}

impl StatefulWidget for CpuMonitor {
    fn update(&mut self) {
        // Update frequencies on cores
        for core in &mut self.stats {
            // TODO: handle err here somehow
            let freq = core.update(self.m.elapsed_since_start()).unwrap();
            self.m.set_if_y_max(freq + 100_000.);
            self.m.set_if_y_min(freq + 100_000.);
        }

        // Move x axis if time reached end
        if self.m.elapsed_since_start() > self.m.max_x() {
            let removed = self.stats[0].data.pop();
            if let Some(point) = self.stats[0].data.first() {
                self.m.inc_x_axis(point.0 - removed.0);
            }
            self.stats.iter_mut().skip(1).for_each(|c| {
                c.data.pop();
            });
        }
    }
    fn render_widget<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(20), Constraint::Min(80)].as_ref())
            .split(area);

        self.render_cores_info_widget(f, chunks[0]);
        self.render_graph_widget(f, chunks[1]);
    }
}

impl GraphWidget for CpuMonitor {
    fn datasets(&self) -> Vec<Dataset> {
        let mut data = Vec::new();
        for core in &self.stats {
            data.push(
                Dataset::default()
                    .name(&core.name)
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(core.color))
                    .data(&core.data.dataset()),
            );
        }
        data
    }
    fn title(&self) -> Span {
        Span::styled("Cpu Frequency", Style::default().add_modifier(Modifier::BOLD))
    }
    fn x_axis(&self) -> Span {
        Span::styled("Time", Style::default().fg(Color::White))
    }
    fn y_axis(&self) -> Span {
        Span::styled("Frequency", Style::default().fg(Color::White))
    }
    fn labels(&self) -> Vec<Span> {
        self.m.bounds_labels(conv_fhz, 4)
    }
    fn monitor(&self) -> &Monitor {
        &self.m
    }
}

impl CpuMonitor {
    pub fn new() -> Result<CpuMonitor> {
        let cpu = processor()?;
        let mut stats = Vec::new();
        for core in cpu.cores {
            stats.push(CoreStat::from(core));
        }
        stats.sort_by(|s1, s2| s1.core.id.cmp(&s2.core.id));

        Ok(CpuMonitor {
            stats,
            m: Monitor::new(X_AXIS, Y_AXIS),
        })
    }

    fn render_cores_info_widget<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(10), Constraint::Percentage(90)])
            .split(area);

        let headers = ["core", "frequency"];
        let data = self.stats.iter().map(|s| {
            Row::StyledData(
                vec![s.name.clone(), conv_hz(s.core.cur_freq)].into_iter(),
                Style::default().fg(s.color),
            )
        });

        let table = Table::new(headers.iter(), data).widths(&[Constraint::Percentage(25), Constraint::Percentage(60)]);

        f.render_widget(table, chunks[1]);
    }

    pub fn graph_loop() -> Result<()> {
        let mut monitor = CpuMonitor::new()?;
        single_widget_loop(&mut monitor, Config::new(TICK_RATE))
    }
}
