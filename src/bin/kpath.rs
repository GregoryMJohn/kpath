use std::{fs::File, io::Write};

use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyModifiers};
use kpath::bravais::{
	self, BravaisLattice, KPoint, KPoints, LatticeParameters,
};
use ratatui::{
	DefaultTerminal, Frame,
	layout::{Constraint, Layout, Margin, Position, Rect},
	style::{Color, Modifier, Style, Stylize, palette::tailwind},
	text::{Line, Span, Text},
	widgets::{
		Block, BorderType, Cell, Clear, HighlightSpacing, List, ListItem,
		Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
		TableState,
	},
};
use unicode_width::UnicodeWidthStr;

const PALETTES: [tailwind::Palette; 4] = [
	tailwind::EMERALD,
	tailwind::BLUE,
	tailwind::INDIGO,
	tailwind::RED,
];

const INFO_TEXT: [&str; 2] = [
	"(q) Quit | (Esc) Back | (↑) Move Up | (↓) Move Down | (←) Move Left | (→) Move Right",
	"(Shift + →) Next Color | (Shift + ←) Previous Color",
];

const ITEM_HEIGHT: usize = 4;

#[derive(Debug, Default, PartialEq, Eq)]
enum RunningState {
	#[default]
	Running,
	Done,
}

#[derive(Default, PartialEq, Eq)]
enum Mode {
	#[default]
	SpaceGroupSelection,
	LatticeParamsInput,
	KPathSelection,
	NumberOfPointsInput,
}

enum Message {
	NextRow,
	PreviousRow,
	NextColumn,
	PreviousColumn,
	NextColor,
	PreviousColor,
	Select,
	DeletePoint,
	Back,
	Quit,
}

fn main() -> Result<()> {
	color_eyre::install()?;
	ratatui::run(|terminal| App::new().run(terminal))
}

struct App {
	running_state:     RunningState,
	mode:              Mode,
	space_group_table: MyTable<SpaceGroup>,
	kpoints_table:     Option<MyTable<KPoint>>,
	text_entry:        TextEntry,
	space_group:       Option<SpaceGroup>,
	lattice_params:    Option<LatticeParameters>,
	bravais:           Option<BravaisLattice>,
	kpath:             Vec<String>,
	nkpoints:          u8,
}

impl App {
	fn new() -> Self {
		let mut text_entry = TextEntry::default();
		text_entry.message_names = vec![
			"a".into(),
			"b".into(),
			"c".into(),
			"α".into(),
			"β".into(),
			"γ".into(),
		];
		Self {
			running_state: RunningState::default(),
			mode: Mode::default(),
			space_group_table: MyTable::<SpaceGroup>::new(),
			kpoints_table: None,
			text_entry,
			space_group: None,
			lattice_params: None,
			bravais: None,
			kpath: vec![],
			nkpoints: 0,
		}
	}

	fn update(&mut self, msg: Message) -> Option<Message> {
		use Message::*;
		use Mode::*;

		match msg {
			| Quit => self.running_state = RunningState::Done,
			| Back => {
				self.mode = match self.mode {
					| SpaceGroupSelection => {
						match self.space_group_table.text_entry.input_mode {
							| InputMode::Editing => {
								self.space_group_table.text_entry.input_mode =
									InputMode::Normal;
								SpaceGroupSelection
							}
							| InputMode::Normal => SpaceGroupSelection,
						}
					}
					| LatticeParamsInput => SpaceGroupSelection,
					| KPathSelection => LatticeParamsInput,
					| NumberOfPointsInput => KPathSelection,
				}
			}
			| NextRow => match self.mode {
				| SpaceGroupSelection => self.space_group_table.next_row(),
				| KPathSelection => {
					self.kpoints_table.as_mut().unwrap().next_row()
				}
				| _ => (),
			},
			| PreviousRow => match self.mode {
				| SpaceGroupSelection => self.space_group_table.previous_row(),
				| KPathSelection => {
					self.kpoints_table.as_mut().unwrap().previous_row()
				}
				| _ => (),
			},
			| NextColumn => match self.mode {
				| SpaceGroupSelection => self.space_group_table.next_column(),
				| KPathSelection => {
					self.kpoints_table.as_mut().unwrap().next_column()
				}
				| _ => (),
			},
			| PreviousColumn => match self.mode {
				| SpaceGroupSelection => {
					self.space_group_table.previous_column()
				}
				| KPathSelection => {
					self.kpoints_table.as_mut().unwrap().previous_column()
				}
				| _ => (),
			},
			| NextColor => self.space_group_table.next_color(),
			| PreviousColor => self.space_group_table.previous_color(),
			| Select => match self.mode {
				| SpaceGroupSelection => {
					if self.space_group_table.text_entry.input_mode
						== InputMode::Normal
					{
						let i =
							match self.space_group_table.table_state.selected()
							{
								| Some(i) => i,
								| None => 0,
							};

						self.space_group =
							Some(self.space_group_table.rows[i].clone());
						self.space_group_table.selected =
							self.space_group.as_ref().unwrap().id() as usize
								- 1;

						self.mode = LatticeParamsInput;
					} else {
						self.space_group_table.text_entry.input_mode =
							InputMode::Normal;
						let id = self
							.space_group_table
							.text_entry
							.messages
							.first()
							.unwrap()
							.parse::<usize>()
							.unwrap();
						self.space_group =
							Some(self.space_group_table.rows[id - 1].clone());
						self.space_group_table.selected =
							self.space_group.as_ref().unwrap().id() as usize
								- 1;
						*self.space_group_table.table_state.selected_mut() =
							Some(self.space_group_table.selected);
						self.mode = LatticeParamsInput;
					}
				}
				| LatticeParamsInput => {
					if self.text_entry.messages.len() < 6 {
						panic!("Not all lattice parameters were provided. ")
					} else {
						let lattice_params = self
							.text_entry
							.messages
							.iter()
							.map(|s| {
								s.parse::<f64>().expect(
									"Non-numerical value provided for lattice parameter",
								)
							})
							.collect::<Vec<_>>();

						let lattice_params =
							lattice_params.as_array::<6>().unwrap();
						let lattice_params =
							LatticeParameters::from(lattice_params);

						let bravais = BravaisLattice::from_space_group_number(
							self.space_group.as_ref().unwrap().id(),
							&lattice_params,
						)
						.unwrap();
						let kpoints = bravais.kpoints(&lattice_params);
						self.bravais = Some(bravais);
						self.lattice_params = Some(lattice_params);
						self.kpoints_table =
							Some(MyTable::<KPoint>::from_kpoints(kpoints));

						self.mode = KPathSelection;
						self.text_entry.clear_messages();
					}
				}
				| KPathSelection => {
					let i = self
						.kpoints_table
						.as_ref()
						.unwrap()
						.table_state
						.selected()
						.unwrap();
					self.kpoints_table.as_mut().unwrap().selected = i;

					let kpoint = self.kpoints_table.as_ref().unwrap().rows[i]
						.name
						.clone();

					self.kpath.push(kpoint.clone());

					self.kpoints_table
						.as_mut()
						.unwrap()
						.text_entry
						.messages
						.push(kpoint);
				}
				| NumberOfPointsInput => {
					self.nkpoints =
						self.text_entry.input.parse::<u8>().unwrap();
					let mut file = File::create("kpath.txt").unwrap();
					let kpoints = self
						.bravais
						.as_ref()
						.unwrap()
						.kpoints(&self.lattice_params.as_ref().unwrap());
					for (i, point) in
						self.kpath[0..self.kpath.len() - 1].iter().enumerate()
					{
						let coord = kpoints.get(point.as_str()).unwrap();

						let _ = writeln!(
							&mut file,
							"{:.8} {:.8} {:.8}",
							coord[0], coord[1], coord[2]
						);

						let next =
							kpoints.get(self.kpath[i + 1].as_str()).unwrap();
						let sepvec = [
							next[0] - coord[0],
							next[1] - coord[1],
							next[2] - coord[2],
						];

						for j in (1..=self.nkpoints).rev() {
							let point = sepvec
								.iter()
								.map(|x| *x / (j as f64))
								.collect::<Vec<_>>();
							let _ = writeln!(
								&mut file,
								"{:.8} {:.8} {:.8}",
								point[0], point[1], point[2]
							);
						}
					}

					return Some(Quit)
				}
			},
			| DeletePoint => {
				self.kpath.pop();
				self.kpoints_table
					.as_mut()
					.unwrap()
					.text_entry
					.messages
					.pop();
			}
		}

		None
	}

	fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
		while self.running_state != RunningState::Done {
			terminal.draw(|frame| self.render(frame))?;

			let mut current_msg = self.handle_event()?;
			while current_msg.is_some() {
				current_msg = self.update(current_msg.unwrap());
			}
		}

		Ok(())
	}

	fn handle_event(&mut self) -> color_eyre::Result<Option<Message>> {
		let msg = if let Some(key) = event::read()?.as_key_press_event() {
			let shift_pressed = key.modifiers.contains(KeyModifiers::SHIFT);
			use KeyCode::*;
			use Message::*;
			use Mode::*;

			let msg = match &self.mode {
				| SpaceGroupSelection => {
					match self.space_group_table.text_entry.input_mode {
						| InputMode::Editing => {
							match key.code {
								| Enter => {
									self.space_group_table
										.text_entry
										.submit_message();

									return Ok(Some(Select));
								}
								| Char(to_insert) => self
									.space_group_table
									.text_entry
									.enter_char(to_insert),
								| Backspace => self
									.space_group_table
									.text_entry
									.delete_char(),
								| Left => self
									.space_group_table
									.text_entry
									.move_cursor_left(),
								| Right => self
									.space_group_table
									.text_entry
									.move_cursor_right(),
								| Esc => {
									self.space_group_table
										.text_entry
										.input_mode = InputMode::Normal
								}
								| _ => {}
							}
							return Ok(None);
						}
						| InputMode::Normal => match key.code {
							| Char('q') => Quit,
							| Esc => Back,
							| Char('e') => {
								self.space_group_table.text_entry.input_mode =
									InputMode::Editing;
								return Ok(None);
							}
							| Char('j') | Down => NextRow,
							| Char('k') | Up => PreviousRow,
							| Char('l') | Right if shift_pressed => NextColor,
							| Char('h') | Left if shift_pressed => {
								PreviousColor
							}
							| Char('l') | Right => NextColumn,
							| Char('h') | Left => PreviousColumn,
							| Enter => Select,
							| _ => return Ok(None),
						},
					}
				}
				| mode @ (LatticeParamsInput | NumberOfPointsInput) => {
					match self.text_entry.input_mode {
						| InputMode::Editing => {
							match key.code {
								| Enter => {
									if *mode == LatticeParamsInput
										&& self.text_entry.messages.len() < 6
									{
										self.text_entry.submit_message()
									} else {
										return Ok(Some(Select))
									}
								}
								| Char(to_insert) => {
									self.text_entry.enter_char(to_insert)
								}
								| Backspace => self.text_entry.delete_char(),
								| Left => self.text_entry.move_cursor_left(),
								| Right => self.text_entry.move_cursor_right(),
								| Esc => {
									self.text_entry.input_mode =
										InputMode::Normal
								}
								| _ => {}
							}
							return Ok(None);
						}
						| InputMode::Normal => match key.code {
							| Char('e') => {
								self.text_entry.input_mode = InputMode::Editing;
								return Ok(None);
							}
							| Char('q') => Quit,
							| Esc => Back,
							| Enter => Select,
							| Char('l') | Right if shift_pressed => NextColor,
							| Char('h') | Left if shift_pressed => {
								PreviousColor
							}
							| _ => return Ok(None),
						},
					}
				}
				| KPathSelection if key.code == Char(' ') => {
					self.mode = NumberOfPointsInput;
					return Ok(None)
				}
				| _ => match key.code {
					| Char('q') => Quit,
					| Esc => Back,
					| Char('j') | Down => NextRow,
					| Char('k') | Up => PreviousRow,
					| Char('l') | Right if shift_pressed => NextColor,
					| Char('h') | Left if shift_pressed => PreviousColor,
					| Char('l') | Right => NextColumn,
					| Char('h') | Left => PreviousColumn,
					| Enter => Select,
					| Backspace => {
						if matches!(self.mode, KPathSelection) {
							DeletePoint
						} else {
							return Ok(None)
						}
					}
					| _ => return Ok(None),
				},
			};
			Some(msg)
		} else {
			None
		};

		Ok(msg)
	}

	fn render(&mut self, frame: &mut Frame) {
		let layout =
			Layout::vertical([Constraint::Min(5), Constraint::Length(4)]);
		let rects = frame.area().layout_vec(&layout);

		self.space_group_table.set_colors();
		let block1 = Block::bordered()
			.border_type(BorderType::Double)
			.border_style(
				Style::new()
					.fg(self.space_group_table.colors.footer_border_color),
			)
			.title("Select a space group");

		let block2 = Block::bordered()
			.border_type(BorderType::Double)
			.border_style(
				Style::new()
					.fg(self.space_group_table.colors.footer_border_color),
			);

		let area1 = block1.inner(rects[0]);
		let area2 = block2.inner(rects[1]);

		frame.render_widget(block1, rects[0]);
		frame.render_widget(block2, rects[1]);

		self.space_group_table.render_table(frame, area1);
		self.space_group_table.render_scrollbar(frame, area1);

		self.render_footer(frame, area2);

		match self.mode {
			| Mode::LatticeParamsInput => {
				self.render_lattice_params_popup(frame, area1)
			}
			| Mode::KPathSelection => self.render_kpath_popup(frame, area1),
			| Mode::NumberOfPointsInput => {
				self.render_nkpoints_popup(frame, area1)
			}
			| _ => {}
		}
	}

	fn render_nkpoints_popup(&mut self, frame: &mut Frame, area: Rect) {
		let area = self.render_popup(frame, area);
		self.text_entry.show_messages = false;
		self.text_entry.input_title = Some("Enter the number of points to be interpolated between each k-point".to_owned());
		self.text_entry.render(frame, area)
	}

	fn render_lattice_params_popup(&mut self, frame: &mut Frame, area: Rect) {
		let area = self.render_popup(frame, area);
		self.text_entry.show_messages = true;
		self.text_entry.message_area_title =
			Some("Lattice Parameters".to_owned());
		self.text_entry.render(frame, area)
	}

	fn render_kpath_popup(&mut self, frame: &mut Frame, area: Rect) {
		let area = self.render_popup(frame, area);
		self.kpoints_table
			.as_mut()
			.unwrap()
			.render_table(frame, area);
	}

	fn render_popup(&self, frame: &mut Frame, area: Rect) -> Rect {
		let centered_area = area
			.centered(Constraint::Percentage(60), Constraint::Percentage(60));

		frame.render_widget(Clear, centered_area);

		let space_group = &self.space_group_table.rows
			[self.space_group_table.selected as usize];

		let popup_block = Block::bordered()
			.style(Style::new().bg(self.space_group_table.colors.alt_row_color))
			.border_type(BorderType::Double)
			.border_style(
				Style::new()
					.fg(self.space_group_table.colors.footer_border_color),
			)
			.title(format!(" {} ", space_group.name()).bold());

		let paragraph = Paragraph::new("").block(popup_block);
		frame.render_widget(paragraph, centered_area);

		centered_area.inner(Margin::new(1, 1))
	}

	fn render_footer(&self, frame: &mut Frame, area: Rect) {
		let info_footer = Paragraph::new(Text::from_iter(INFO_TEXT))
			.style(
				Style::new()
					.fg(self.space_group_table.colors.row_fg)
					.bg(self.space_group_table.colors.buffer_bg),
			)
			.centered();

		frame.render_widget(info_footer, area);
	}
}

fn constraint_len_calculator<T>(data: &[T]) -> Vec<u16>
where
	T: StringifyFields,
{
	let mut cols: Vec<Vec<usize>> =
		(0..T::num_fields()).map(|_| Vec::new()).collect();

	let data = data
		.iter()
		.map(|d| d.to_strings())
		.map(|v| {
			v.iter()
				.map(|s| UnicodeWidthStr::width(s.as_str()))
				.collect::<Vec<_>>()
		})
		.collect::<Vec<_>>();

	for row in data {
		for (i, elem) in row.iter().enumerate() {
			cols[i].push(*elem);
		}
	}

	cols.iter()
		.map(|v| *v.iter().max().unwrap() as u16)
		.collect()
}

trait StringifyFields {
	fn to_strings(&self) -> Vec<String>;
	fn num_fields() -> usize;
}

struct MyTable<T> {
	selected:          usize,
	table_state:       TableState,
	rows:              Vec<T>,
	longest_item_lens: Vec<u16>,
	scroll_state:      ScrollbarState,
	colors:            TableColors,
	color_index:       usize,
	text_entry:        TextEntry,
}

impl MyTable<SpaceGroup> {
	fn new() -> Self {
		let space_group_names = bravais::space_groups();

		let space_groups = (1..=230)
			.zip(space_group_names)
			.map(|(a, b)| SpaceGroup {
				space_group_id: a.to_string(),
				name:           String::from(b),
			})
			.collect::<Vec<_>>();

		Self {
			selected:          0,
			table_state:       TableState::default().with_selected(0),
			longest_item_lens: constraint_len_calculator(&space_groups),
			scroll_state:      ScrollbarState::new(
				(space_groups.len() - 1) * ITEM_HEIGHT,
			),
			colors:            TableColors::new(&PALETTES[0]),
			color_index:       0,
			rows:              space_groups,
			text_entry:        TextEntry::default(),
		}
	}

	fn render_table(&mut self, frame: &mut Frame, area: Rect) {
		let header_style = Style::default()
			.fg(self.colors.header_fg)
			.bg(self.colors.header_bg);

		let selected_row_style = Style::default()
			.add_modifier(Modifier::REVERSED)
			.fg(self.colors.selected_row_style_fg);

		let selected_col_style =
			Style::default().fg(self.colors.selected_column_style_fg);
		let selected_cell_style = Style::default()
			.add_modifier(Modifier::REVERSED)
			.fg(self.colors.selected_cell_style_fg);

		let header = ["Id", "Name"]
			.into_iter()
			.map(Cell::from)
			.collect::<Row>()
			.style(header_style)
			.height(1);

		let rows = self.rows.iter().enumerate().map(|(i, data)| {
			let color = match i % 2 {
				| 0 => self.colors.normal_row_color,
				| _ => self.colors.alt_row_color,
			};

			let item = data.to_strings();
			item.into_iter()
				.map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
				.collect::<Row>()
				.style(Style::new().fg(self.colors.row_fg).bg(color))
				.height(4)
		});

		let bar = " █ ";
		let t = Table::new(
			rows,
			[
				Constraint::Length(self.longest_item_lens[0] + 1),
				Constraint::Min(self.longest_item_lens[1] + 1),
			],
		)
		.header(header)
		.row_highlight_style(selected_row_style)
		.column_highlight_style(selected_col_style)
		.cell_highlight_style(selected_cell_style)
		.highlight_symbol(Text::from(vec![
			"".into(),
			bar.into(),
			bar.into(),
			"".into(),
		]))
		.bg(self.colors.buffer_bg)
		.highlight_spacing(HighlightSpacing::Always);

		let layout =
			Layout::vertical([Constraint::Length(5), Constraint::Min(3)]);
		let [input_area, table_area] = area.layout(&layout);

		self.text_entry.show_messages = false;
		self.text_entry.render(frame, input_area);
		frame.render_stateful_widget(t, table_area, &mut self.table_state);
	}
}

impl MyTable<KPoint> {
	fn from_kpoints(kpoints: KPoints) -> Self {
		let kpoints: Vec<KPoint> = kpoints
			.iter()
			.map(|(k, v)| KPoint {
				name:  k.to_string(),
				coord: *v,
			})
			.collect();

		Self {
			selected:          0,
			table_state:       TableState::default().with_selected(0),
			longest_item_lens: constraint_len_calculator(&kpoints),
			scroll_state:      ScrollbarState::new(
				(kpoints.len() - 1) * ITEM_HEIGHT,
			),
			colors:            TableColors::new(&PALETTES[0]),
			color_index:       0,
			rows:              kpoints,
			text_entry:        TextEntry::default(),
		}
	}

	fn render_table(&mut self, frame: &mut Frame, area: Rect) {
		let header_style = Style::default()
			.fg(self.colors.header_fg)
			.bg(self.colors.header_bg);

		let selected_row_style = Style::default()
			.add_modifier(Modifier::REVERSED)
			.fg(self.colors.selected_row_style_fg);

		let selected_col_style =
			Style::default().fg(self.colors.selected_column_style_fg);
		let selected_cell_style = Style::default()
			.add_modifier(Modifier::REVERSED)
			.fg(self.colors.selected_cell_style_fg);

		let header = ["Symbol", "kx", "ky", "kz"]
			.into_iter()
			.map(Cell::from)
			.collect::<Row>()
			.style(header_style)
			.height(1);

		let rows = self.rows.iter().enumerate().map(|(i, data)| {
			let color = match i % 2 {
				| 0 => self.colors.normal_row_color,
				| _ => self.colors.alt_row_color,
			};

			let item = data.to_strings();
			item.into_iter()
				.map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
				.collect::<Row>()
				.style(Style::new().fg(self.colors.row_fg).bg(color))
				.height(4)
		});

		let bar = " █ ";
		let t = Table::new(
			rows,
			[
				Constraint::Min(self.longest_item_lens[0] + 1),
				Constraint::Min(self.longest_item_lens[1] + 1),
				Constraint::Min(self.longest_item_lens[2] + 1),
				Constraint::Min(self.longest_item_lens[3]),
			],
		)
		.header(header)
		.row_highlight_style(selected_row_style)
		.column_highlight_style(selected_col_style)
		.cell_highlight_style(selected_cell_style)
		.highlight_symbol(Text::from(vec![
			"".into(),
			bar.into(),
			bar.into(),
			"".into(),
		]))
		.bg(self.colors.buffer_bg)
		.highlight_spacing(HighlightSpacing::Always);

		let layout = Layout::vertical([
			Constraint::Length(3),
			Constraint::Min(3),
			Constraint::Length(5),
		]);
		let [text_area, table_area, help_area] = area.layout(&layout);

		let text = Paragraph::new(self.text_entry.messages.join("-"))
			.centered()
			.style(Style::new().fg(self.colors.selected_row_style_fg));
		frame.render_widget(text, text_area);

		frame.render_stateful_widget(t, table_area, &mut self.table_state);

		let help_block = Block::bordered()
			.border_type(BorderType::Double)
			.border_style(Style::new().fg(self.colors.footer_border_color));
		frame.render_widget(&help_block, help_area);

		let help = Paragraph::new(
			"(Enter) Select k-point | (Spacebar) Confirm k-path",
		)
		.style(
			Style::new()
				.fg(self.colors.row_fg)
				.bg(self.colors.buffer_bg),
		)
		.centered();
		frame.render_widget(help, help_block.inner(help_area))
	}
}

impl<T> MyTable<T>
where
	T: StringifyFields,
{
	pub const fn next_row(&mut self) {
		let i = match self.table_state.selected() {
			| Some(i) => {
				if i >= self.rows.len() - 1 {
					0
				} else {
					i + 1
				}
			}
			| None => 0,
		};

		self.table_state.select(Some(i));
		self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
	}

	pub const fn previous_row(&mut self) {
		let i = match self.table_state.selected() {
			| Some(i) => {
				if i == 0 {
					self.rows.len() - 1
				} else {
					i - 1
				}
			}
			| None => 0,
		};
		self.table_state.select(Some(i));
		self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
	}

	pub fn next_column(&mut self) {
		self.table_state.select_next_column();
	}

	pub fn previous_column(&mut self) {
		self.table_state.select_previous_column();
	}

	pub const fn next_color(&mut self) {
		self.color_index = (self.color_index + 1) % PALETTES.len();
	}

	pub const fn previous_color(&mut self) {
		let count = PALETTES.len();
		self.color_index = (self.color_index + count - 1) % count;
	}

	pub const fn set_colors(&mut self) {
		self.colors = TableColors::new(&PALETTES[self.color_index]);
	}

	fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
		frame.render_stateful_widget(
			Scrollbar::default()
				.orientation(ScrollbarOrientation::VerticalRight)
				.begin_symbol(None)
				.end_symbol(None),
			area.inner(Margin {
				vertical:   1,
				horizontal: 1,
			}),
			&mut self.scroll_state,
		);
	}
}

struct TableColors {
	buffer_bg:                Color,
	header_bg:                Color,
	header_fg:                Color,
	row_fg:                   Color,
	selected_row_style_fg:    Color,
	selected_column_style_fg: Color,
	selected_cell_style_fg:   Color,
	normal_row_color:         Color,
	alt_row_color:            Color,
	footer_border_color:      Color,
}

impl TableColors {
	const fn new(color: &tailwind::Palette) -> Self {
		Self {
			buffer_bg:                tailwind::SLATE.c950,
			header_bg:                color.c900,
			header_fg:                tailwind::SLATE.c200,
			row_fg:                   tailwind::SLATE.c200,
			selected_row_style_fg:    color.c400,
			selected_column_style_fg: color.c400,
			selected_cell_style_fg:   color.c600,
			normal_row_color:         tailwind::SLATE.c950,
			alt_row_color:            tailwind::SLATE.c900,
			footer_border_color:      color.c400,
		}
	}
}

#[derive(Clone)]
struct SpaceGroup {
	space_group_id: String,
	name:           String,
}

impl SpaceGroup {
	fn name(&self) -> &str {
		&self.name
	}

	fn id(&self) -> u8 {
		self.space_group_id.parse::<u8>().unwrap()
	}
}

impl StringifyFields for SpaceGroup {
	fn to_strings(&self) -> Vec<String> {
		let id = self.space_group_id.to_string();
		vec![id, self.name.clone()]
	}

	fn num_fields() -> usize {
		2
	}
}

#[derive(Default)]
struct TextEntry {
	input:              String,
	character_index:    usize,
	input_mode:         InputMode,
	messages:           Vec<String>,
	message_area_title: Option<String>,
	show_messages:      bool,
	input_title:        Option<String>,
	message_names:      Vec<String>,
}

impl TextEntry {
	const fn reset_cursor(&mut self) {
		self.character_index = 0;
	}

	fn submit_message(&mut self) {
		self.messages.push(self.input.clone());
		self.input.clear();
		self.reset_cursor();
	}

	fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
		new_cursor_pos.clamp(0, self.input.chars().count())
	}

	fn move_cursor_left(&mut self) {
		let cursor_moved_left = self.character_index.saturating_sub(1);
		self.character_index = self.clamp_cursor(cursor_moved_left);
	}

	fn move_cursor_right(&mut self) {
		let cursor_moved_right = self.character_index.saturating_add(1);
		self.character_index = self.clamp_cursor(cursor_moved_right);
	}

	fn byte_index(&self) -> usize {
		self.input
			.char_indices()
			.map(|(i, _)| i)
			.nth(self.character_index)
			.unwrap_or(self.input.len())
	}

	fn enter_char(&mut self, new_char: char) {
		let index = self.byte_index();
		self.input.insert(index, new_char);
		self.move_cursor_right();
	}

	fn delete_char(&mut self) {
		let is_not_cursor_leftmost = self.character_index != 0;
		if is_not_cursor_leftmost {
			let current_index = self.character_index;
			let from_left_to_current_index = current_index - 1;

			let before_char_to_delete =
				self.input.chars().take(from_left_to_current_index);
			let after_char_to_delete = self.input.chars().skip(current_index);

			self.input =
				before_char_to_delete.chain(after_char_to_delete).collect();
			self.move_cursor_left();
		}
	}

	fn render(&self, frame: &mut Frame, area: Rect) {
		let layout = Layout::vertical([
			Constraint::Length(1),
			Constraint::Length(3),
			Constraint::Min(1),
		]);

		let [help_area, input_area, messages_area] = area.layout(&layout);

		let (msg, style) = match self.input_mode {
			| InputMode::Normal => (
				vec![
					"Press ".into(),
					"Esc".bold(),
					" to exit, ".into(),
					"e".bold(),
					" to start editing".bold(),
				],
				Style::default().add_modifier(Modifier::RAPID_BLINK),
			),
			| InputMode::Editing => (
				vec![
					"Press ".into(),
					"Esc".bold(),
					" to stop editing,".into(),
					"Enter".bold(),
					" to set the parameter value".into(),
				],
				Style::default(),
			),
		};

		let text = Text::from(Line::from(msg)).patch_style(style);
		let help_message = Paragraph::new(text);
		frame.render_widget(help_message, help_area);

		let input = Paragraph::new(self.input.as_str())
			.style(match self.input_mode {
				| InputMode::Normal => Style::default(),
				| InputMode::Editing => Style::default().fg(Color::Yellow),
			})
			.block(if let Some(input) = self.input_title.clone() {
				Block::bordered().title(input)
			} else {
				Block::bordered()
			});

		frame.render_widget(input, input_area);

		match self.input_mode {
			| InputMode::Normal => {}
			| InputMode::Editing => frame.set_cursor_position(Position::new(
				input_area.x + self.character_index as u16 + 1,
				input_area.y + 1,
			)),
		}

		let messages: Vec<ListItem> = self
			.message_names
			.iter()
			.zip(self.messages.iter())
			.map(|(i, m)| {
				let content = Line::from(Span::raw(format!("{i}) {m}")));
				ListItem::new(content)
			})
			.collect();

		if self.show_messages {
			let messages = List::new(messages).block(
				if let Some(title) = self.message_area_title.clone() {
					Block::bordered().title(title)
				} else {
					Block::bordered()
				},
			);
			frame.render_widget(messages, messages_area);
		}
	}

	fn clear_messages(&mut self) {
		self.messages.clear()
	}
}

#[derive(Default, PartialEq, Eq)]
enum InputMode {
	#[default]
	Normal,
	Editing,
}

impl StringifyFields for KPoint {
	fn to_strings(&self) -> Vec<String> {
		let coord =
			self.coord.iter().map(|c| c.to_string()).collect::<Vec<_>>();
		let mut res = vec![self.name.clone()];
		res.extend(coord);
		res
	}

	fn num_fields() -> usize {
		4
	}
}
