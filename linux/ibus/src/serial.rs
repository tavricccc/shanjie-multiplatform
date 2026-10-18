//! IBus serializable objects (ibusserializable.c and friends). Every one is a D-Bus struct that
//! starts with its type name and an a{sv} of attachments, and is always sent wrapped in a variant.
use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Type, Value};

pub type Attachments = HashMap<String, OwnedValue>;

fn owned<'a>(value: impl Into<Value<'a>>) -> OwnedValue {
    value.into().try_to_owned().expect("IBus objects hold no file descriptors")
}

// IBusAttribute types and values (ibusattribute.h).
pub const ATTR_UNDERLINE: u32 = 1;
pub const ATTR_BACKGROUND: u32 = 3;
pub const UNDERLINE_SINGLE: u32 = 1;

#[derive(Type, Value, OwnedValue)]
pub struct Attribute {
    name: String,
    attachments: Attachments,
    kind: u32,
    value: u32,
    start: u32,
    end: u32,
}

#[derive(Type, Value, OwnedValue)]
pub struct AttrList {
    name: String,
    attachments: Attachments,
    attributes: Vec<OwnedValue>,
}

#[derive(Type, Value, OwnedValue)]
pub struct Text {
    name: String,
    attachments: Attachments,
    text: String,
    attrs: OwnedValue,
}

impl Text {
    pub fn new(text: &str) -> Self {
        Self::with_attributes(text, Vec::new())
    }
    /// `spans` are (type, value, start, end) in characters, as IBus counts them.
    pub fn with_attributes(text: &str, spans: Vec<(u32, u32, u32, u32)>) -> Self {
        let attributes = spans
            .into_iter()
            .map(|(kind, value, start, end)| {
                owned(Attribute { name: "IBusAttribute".into(), attachments: Attachments::new(), kind, value, start, end })
            })
            .collect();
        let attrs = owned(AttrList { name: "IBusAttrList".into(), attachments: Attachments::new(), attributes });
        Self { name: "IBusText".into(), attachments: Attachments::new(), text: text.into(), attrs }
    }
    pub fn value(self) -> OwnedValue {
        owned(self)
    }
}

// IBusOrientation (ibustypes.h).
pub const ORIENTATION_HORIZONTAL: i32 = 0;
pub const ORIENTATION_VERTICAL: i32 = 1;

#[derive(Type, Value, OwnedValue)]
pub struct LookupTable {
    name: String,
    attachments: Attachments,
    page_size: u32,
    cursor_pos: u32,
    cursor_visible: bool,
    round: bool,
    orientation: i32,
    candidates: Vec<OwnedValue>,
    labels: Vec<OwnedValue>,
}

impl LookupTable {
    /// One page exactly as the core shows it: the shell never lets IBus page on its own.
    pub fn page(candidates: &[String], labels: &[String], cursor: Option<u32>, orientation: i32) -> Self {
        let page_size = candidates.len().max(1) as u32;
        Self {
            name: "IBusLookupTable".into(),
            attachments: Attachments::new(),
            page_size,
            cursor_pos: cursor.unwrap_or(0),
            cursor_visible: cursor.is_some(),
            round: false,
            orientation,
            candidates: candidates.iter().map(|c| Text::new(c).value()).collect(),
            labels: labels.iter().map(|l| Text::new(l).value()).collect(),
        }
    }
    pub fn value(self) -> OwnedValue {
        owned(self)
    }
}

// IBusPropType and IBusPropState (ibusproperty.h).
pub const PROP_TYPE_NORMAL: u32 = 0;
pub const PROP_STATE_UNCHECKED: u32 = 0;

#[derive(Type, Value, OwnedValue)]
pub struct Property {
    name: String,
    attachments: Attachments,
    key: String,
    kind: u32,
    label: OwnedValue,
    icon: String,
    tooltip: OwnedValue,
    sensitive: bool,
    visible: bool,
    state: u32,
    sub_props: OwnedValue,
    symbol: OwnedValue,
}

#[derive(Type, Value, OwnedValue)]
pub struct PropList {
    name: String,
    attachments: Attachments,
    properties: Vec<OwnedValue>,
}

impl PropList {
    pub fn new(properties: Vec<Property>) -> Self {
        Self { name: "IBusPropList".into(), attachments: Attachments::new(), properties: properties.into_iter().map(owned).collect() }
    }
    pub fn value(self) -> OwnedValue {
        owned(self)
    }
}

impl Property {
    /// A plain button; gnome-shell shows `symbol` of the "InputMode" property in the top bar.
    pub fn button(key: &str, label: &str, tooltip: &str, symbol: &str) -> Self {
        Self {
            name: "IBusProperty".into(),
            attachments: Attachments::new(),
            key: key.into(),
            kind: PROP_TYPE_NORMAL,
            label: Text::new(label).value(),
            icon: String::new(),
            tooltip: Text::new(tooltip).value(),
            sensitive: true,
            visible: true,
            state: PROP_STATE_UNCHECKED,
            sub_props: PropList::new(Vec::new()).value(),
            symbol: Text::new(symbol).value(),
        }
    }
    pub fn value(self) -> OwnedValue {
        owned(self)
    }
}

#[derive(Type, Value, OwnedValue)]
pub struct EngineDesc {
    name: String,
    attachments: Attachments,
    engine: String,
    longname: String,
    description: String,
    language: String,
    license: String,
    author: String,
    icon: String,
    layout: String,
    rank: u32,
    hotkeys: String,
    symbol: String,
    setup: String,
    layout_variant: String,
    layout_option: String,
    version: String,
    textdomain: String,
    icon_prop_key: String,
}

#[derive(Type, Value, OwnedValue)]
pub struct Component {
    name: String,
    attachments: Attachments,
    component: String,
    description: String,
    version: String,
    license: String,
    author: String,
    homepage: String,
    exec: String,
    textdomain: String,
    observed_paths: Vec<OwnedValue>,
    engines: Vec<OwnedValue>,
}

pub struct Identity<'a> {
    pub component: &'a str,
    pub engine: &'a str,
    pub longname: &'a str,
    pub description: &'a str,
    pub version: &'a str,
    pub exec: &'a str,
    pub symbol: &'a str,
    pub setup: &'a str,
}

impl Identity<'_> {
    /// The same component the installed XML describes, for registering a development build at runtime.
    pub fn component(&self) -> OwnedValue {
        let engine = EngineDesc {
            name: "IBusEngineDesc".into(),
            attachments: Attachments::new(),
            engine: self.engine.into(),
            longname: self.longname.into(),
            description: self.description.into(),
            language: "zh_TW".into(),
            license: "Apache-2.0".into(),
            author: "Shanjie contributors".into(),
            icon: String::new(),
            layout: "us".into(),
            rank: 0,
            hotkeys: String::new(),
            symbol: self.symbol.into(),
            setup: self.setup.into(),
            layout_variant: String::new(),
            layout_option: String::new(),
            version: self.version.into(),
            textdomain: String::new(),
            icon_prop_key: "InputMode".into(),
        };
        owned(Component {
            name: "IBusComponent".into(),
            attachments: Attachments::new(),
            component: self.component.into(),
            description: self.description.into(),
            version: self.version.into(),
            license: "Apache-2.0".into(),
            author: "Shanjie contributors".into(),
            homepage: "https://github.com/Nanako0129/shanjie".into(),
            exec: self.exec.into(),
            textdomain: String::new(),
            observed_paths: Vec::new(),
            engines: vec![owned(engine)],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Type;

    #[test]
    fn signatures_match_ibus() {
        assert_eq!(Text::SIGNATURE.to_string(), "(sa{sv}sv)");
        assert_eq!(AttrList::SIGNATURE.to_string(), "(sa{sv}av)");
        assert_eq!(Attribute::SIGNATURE.to_string(), "(sa{sv}uuuu)");
        assert_eq!(LookupTable::SIGNATURE.to_string(), "(sa{sv}uubbiavav)");
        assert_eq!(Property::SIGNATURE.to_string(), "(sa{sv}suvsvbbuvv)");
        assert_eq!(PropList::SIGNATURE.to_string(), "(sa{sv}av)");
        assert_eq!(EngineDesc::SIGNATURE.to_string(), "(sa{sv}ssssssssussssssss)");
        assert_eq!(Component::SIGNATURE.to_string(), "(sa{sv}ssssssssavav)");
    }
}

#[cfg(test)]
mod dump {
    #[test]
    #[ignore]
    fn print_objects() {
        let table = super::LookupTable::page(&["測".into(), "試".into()], &["1".into(), "2".into()], Some(0), 0).value();
        println!("TABLE {}", zbus::zvariant::Value::from(table));
        println!("TEXT {}", zbus::zvariant::Value::from(super::Text::with_attributes("ab", vec![(1, 1, 0, 2)]).value()));
    }
}
