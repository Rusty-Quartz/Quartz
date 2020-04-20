pub enum Color {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
    Reset
}

impl Color {
    pub fn name(&self) -> &str {
        match self {
            Color::Black => "black",
            Color::DarkBlue => "dark_blue",
            Color::DarkGreen => "dark_green",
            Color::DarkAqua => "dark_aqua",
            Color::DarkRed => "dark_red",
            Color::DarkPurple => "dark_purple",
            Color::Gold => "gold",
            Color::Gray => "gray",
            Color::DarkGray => "dark_gray",
            Color::Blue => "blue",
            Color::Green => "green",
            Color::Aqua => "aqua",
            Color::Red => "red",
            Color::LightPurple => "light_purple",
            Color::Yellow => "yellow",
            Color::White => "white",
            Color::Reset => "reset"
        }
    }
}

pub enum Format {
    Obfuscated(bool),
    Bold(bool),
    Strikethrough(bool),
    Underline(bool),
    Italic(bool)
}

impl Format {
    pub fn to_json_pair(&self) -> String {
        match self {
            Format::Obfuscated(value) => format!("\"obfuscated\":{}", value),
            Format::Bold(value) => format!("\"bold\":{}", value),
            Format::Strikethrough(value) => format!("\"strikethrough\":{}", value),
            Format::Underline(value) => format!("\"underline\":{}", value),
            Format::Italic(value) => format!("\"italic\":{}", value)
        }
    }
}

macro_rules! event_format {
    ($action:expr, $value:expr) => {
        format!(concat!("{{\"action\":\"", $action, "\",\"value\":\"{}\"}}"), $value)
    };
}

macro_rules! escape {
    ($string:expr) => {
        $string.replace("\\", "\\\\").replace("\"", "\\\"")
    };
}

#[repr(transparent)]
pub struct Message(Vec<Component>);

pub struct Component {
    text: String,
    color: Option<Color>,
    formatting: Option<Vec<Format>>,
    insertion: Option<String>,
    click_event: Option<Box<ClickEvent>>,
    hover_event: Option<Box<HoverEvent>>,
    children: Option<Vec<Component>>
}

impl Component {
    pub fn to_json(&self) -> String {
        let mut json = String::from("{\"text\":\"");
        json.push_str(&escape!(self.text));
        json.push('\"');

        if let Some(color) = &self.color {
            json.push_str(",\"color\":\"");
            json.push_str(color.name());
            json.push('\"');
        }

        if let Some(formatting) = &self.formatting {
            for format in formatting {
                json.push(',');
                json.push_str(&format.to_json_pair());
            }
        }

        if let Some(insertion) = &self.insertion {
            json.push_str(",\"insertion\":\"");
            json.push_str(&escape!(insertion));
            json.push('\"');
        }

        if let Some(click_event) = &self.click_event {
            json.push_str(",\"clickEvent\":");
            json.push_str(&click_event.to_json());
        }

        if let Some(hover_event) = &self.hover_event {
            json.push_str(",\"hoverEvent\":");
            json.push_str(&hover_event.to_json());
        }

        if let Some(children) = &self.children {
            json.push('[');
            json.push_str(&children[0].to_json());
            for child in children.iter().skip(1) {
                json.push(',');
                json.push_str(&child.to_json());
            }
            json.push(']');
        }

        json.push('}');
        json
    }
}

pub enum ClickEvent {
    OpenUrl(String),
    RunCommand(String),
    SuggestCommand(String),
    ChangePage(u32)
}

impl ClickEvent {
    pub fn to_json(&self) -> String {
        match self {
            ClickEvent::OpenUrl(url) => event_format!("open_url", escape!(url)),
            ClickEvent::RunCommand(command) => event_format!("run_command", escape!(command)),
            ClickEvent::SuggestCommand(command) => event_format!("suggest_command", escape!(command)),
            ClickEvent::ChangePage(page) => event_format!("change_page", page)
        }
    }
}

pub enum HoverEvent {
    ShowText(Component),
    ShowItem(String),
    ShowEntity(String)
}

impl HoverEvent {
    pub fn to_json(&self) -> String {
        match self {
            HoverEvent::ShowText(component) => format!("{{\"action\":\"show_text\",\"value\":{}}}", component.to_json()),
            HoverEvent::ShowItem(snbt) => event_format!("show_item", escape!(snbt)),
            HoverEvent::ShowEntity(snbt) => event_format!("show_entity", escape!(snbt))
        }
    }
}