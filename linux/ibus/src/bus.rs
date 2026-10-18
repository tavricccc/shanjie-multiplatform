//! The IBus D-Bus side: org.freedesktop.IBus.Factory and org.freedesktop.IBus.Engine (ibusengine.c,
//! ibusfactory.c). Every engine object owns one `Session`; the session returns its visible effects and
//! this module turns them into the engine signals in order.
use crate::serial::{self, LookupTable, PropList, Property, Text};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use zbus::{
    interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value},
    Connection,
};

// IBusCapabilite (ibustypes.h).
pub const CAP_PREEDIT_TEXT: u32 = 1 << 0;
pub const CAP_SURROUNDING_TEXT: u32 = 1 << 5;
// IBusPreeditFocusMode: with COMMIT the daemon or the client (ClientCommitPreedit) commits the shown
// preedit on focus out and reset, exactly once; the session then only clears its state.
const PREEDIT_COMMIT: u32 = 1;

pub enum Effect {
    Commit(String),
    /// Text, cursor in characters, (type, value, start, end) attribute spans in characters.
    Preedit(String, u32, Vec<(u32, u32, u32, u32)>),
    HidePreedit,
    /// Candidates, labels, highlighted index, vertical.
    Lookup(Vec<String>, Vec<String>, Option<u32>, bool),
    HideLookup,
    Auxiliary(String),
    HideAuxiliary,
    Properties(Vec<Property>),
    UpdateProperty(Property),
    RequireSurrounding,
    DeleteSurrounding(i32, u32),
    Forward(u32, u32, u32),
}

pub type Effects = Vec<Effect>;

/// One input context's state machine. Methods only record effects; nothing here touches D-Bus.
pub trait Session: Send + Sync + 'static {
    fn key(&mut self, keyval: u32, keycode: u32, state: u32, out: &mut Effects) -> bool;
    fn focus_in(&mut self, out: &mut Effects);
    fn focus_out(&mut self, out: &mut Effects);
    fn reset(&mut self, out: &mut Effects);
    fn enable(&mut self, out: &mut Effects);
    fn disable(&mut self, out: &mut Effects);
    fn capabilities(&mut self, caps: u32);
    fn content_type(&mut self, _purpose: u32, _hints: u32) {}
    fn surrounding(&mut self, text: &str, cursor: u32, anchor: u32, out: &mut Effects);
    fn candidate_clicked(&mut self, index: u32, out: &mut Effects);
    fn page(&mut self, down: bool, out: &mut Effects);
    fn cursor(&mut self, down: bool, out: &mut Effects);
    fn property(&mut self, name: &str, state: u32, out: &mut Effects);
    /// Work finished off the D-Bus thread (see `Waker`); called on the engine object.
    fn wake(&mut self, _out: &mut Effects) {}
}

/// Lets background work ask its engine object to call `Session::wake` and emit the result.
#[derive(Clone)]
pub struct Waker {
    connection: Connection,
    path: OwnedObjectPath,
}

impl Waker {
    pub fn wake(&self) {
        let connection = self.connection.clone();
        let path = self.path.clone();
        self.connection
            .executor()
            .spawn(
                async move {
                    let server = connection.object_server();
                    let Ok(iface) = server.interface::<_, EngineObject>(&path).await else { return };
                    let emitter = iface.signal_emitter().clone();
                    let mut engine = iface.get_mut().await;
                    let mut out = Effects::new();
                    engine.session.wake(&mut out);
                    emit(&emitter, out).await;
                },
                "shanjie wake",
            )
            .detach();
    }
}

pub type Maker = Arc<dyn Fn(Waker) -> Box<dyn Session> + Send + Sync>;

async fn emit(e: &SignalEmitter<'_>, effects: Effects) {
    for effect in effects {
        let result = match effect {
            Effect::Commit(text) => EngineObject::commit_text(e, Text::new(&text).value()).await,
            Effect::Preedit(text, cursor, spans) => {
                let visible = !text.is_empty();
                EngineObject::update_preedit_text(e, Text::with_attributes(&text, spans).value(), cursor, visible, PREEDIT_COMMIT).await
            }
            Effect::HidePreedit => EngineObject::update_preedit_text(e, Text::new("").value(), 0, false, PREEDIT_COMMIT).await,
            Effect::Lookup(candidates, labels, cursor, vertical) => {
                let orientation = if vertical { serial::ORIENTATION_VERTICAL } else { serial::ORIENTATION_HORIZONTAL };
                let visible = !candidates.is_empty();
                EngineObject::update_lookup_table(e, LookupTable::page(&candidates, &labels, cursor, orientation).value(), visible).await
            }
            Effect::HideLookup => EngineObject::update_lookup_table(e, LookupTable::page(&[], &[], None, 0).value(), false).await,
            Effect::Auxiliary(text) => EngineObject::update_auxiliary_text(e, Text::new(&text).value(), true).await,
            Effect::HideAuxiliary => EngineObject::update_auxiliary_text(e, Text::new("").value(), false).await,
            Effect::Properties(props) => EngineObject::register_properties(e, PropList::new(props).value()).await,
            Effect::UpdateProperty(prop) => EngineObject::update_property(e, prop.value()).await,
            Effect::RequireSurrounding => EngineObject::require_surrounding_text(e).await,
            Effect::DeleteSurrounding(offset, chars) => EngineObject::delete_surrounding_text(e, offset, chars).await,
            Effect::Forward(keyval, keycode, state) => EngineObject::forward_key_event(e, keyval, keycode, state).await,
        };
        if let Err(error) = result {
            eprintln!("ibus-engine-shanjie: signal failed: {error}");
        }
    }
}

pub struct EngineObject {
    session: Box<dyn Session>,
}

#[interface(name = "org.freedesktop.IBus.Engine")]
impl EngineObject {
    async fn process_key_event(&mut self, keyval: u32, keycode: u32, state: u32, #[zbus(signal_emitter)] e: SignalEmitter<'_>) -> bool {
        let mut out = Effects::new();
        let handled = self.session.key(keyval, keycode, state, &mut out);
        emit(&e, out).await;
        handled
    }
    async fn set_cursor_location(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}
    async fn set_cursor_location_relative(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}
    async fn process_hand_writing_event(&self, _coordinates: Vec<f64>) {}
    async fn cancel_hand_writing(&self, _strokes: u32) {}
    async fn set_capabilities(&mut self, caps: u32) {
        self.session.capabilities(caps);
    }
    async fn property_activate(&mut self, name: String, state: u32, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.property(&name, state, &mut out);
        emit(&e, out).await;
    }
    async fn property_show(&self, _name: String) {}
    async fn property_hide(&self, _name: String) {}
    async fn candidate_clicked(&mut self, index: u32, _button: u32, _state: u32, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.candidate_clicked(index, &mut out);
        emit(&e, out).await;
    }
    async fn focus_in(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.focus_in(&mut out);
        emit(&e, out).await;
    }
    async fn focus_in_id(&mut self, _object_path: String, _client: String, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        self.focus_in(e).await;
    }
    async fn focus_out(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.focus_out(&mut out);
        emit(&e, out).await;
    }
    async fn focus_out_id(&mut self, _object_path: String, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        self.focus_out(e).await;
    }
    async fn reset(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.reset(&mut out);
        emit(&e, out).await;
    }
    async fn enable(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.enable(&mut out);
        emit(&e, out).await;
    }
    async fn disable(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.disable(&mut out);
        emit(&e, out).await;
    }
    async fn page_up(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.page(false, &mut out);
        emit(&e, out).await;
    }
    async fn page_down(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.page(true, &mut out);
        emit(&e, out).await;
    }
    async fn cursor_up(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.cursor(false, &mut out);
        emit(&e, out).await;
    }
    async fn cursor_down(&mut self, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.cursor(true, &mut out);
        emit(&e, out).await;
    }
    async fn set_surrounding_text(&mut self, text: OwnedValue, cursor_pos: u32, anchor_pos: u32, #[zbus(signal_emitter)] e: SignalEmitter<'_>) {
        let mut out = Effects::new();
        self.session.surrounding(&text_of(&text), cursor_pos, anchor_pos, &mut out);
        emit(&e, out).await;
    }
    async fn panel_extension_received(&self, _event: OwnedValue) {}
    async fn panel_extension_register_keys(&self, _data: OwnedValue) {}

    #[zbus(property)]
    async fn set_content_type(&mut self, value: (u32, u32)) {
        self.session.content_type(value.0, value.1);
    }
    /// false: this engine wants plain FocusIn / FocusOut.
    #[zbus(property)]
    async fn focus_id(&self) -> (bool,) {
        (false,)
    }
    #[zbus(property)]
    async fn active_surrounding_text(&self) -> (bool,) {
        (false,)
    }

    #[zbus(signal)]
    async fn commit_text(e: &SignalEmitter<'_>, text: OwnedValue) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn forward_key_event(e: &SignalEmitter<'_>, keyval: u32, keycode: u32, state: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn update_preedit_text(e: &SignalEmitter<'_>, text: OwnedValue, cursor_pos: u32, visible: bool, mode: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn update_auxiliary_text(e: &SignalEmitter<'_>, text: OwnedValue, visible: bool) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn update_lookup_table(e: &SignalEmitter<'_>, table: OwnedValue, visible: bool) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn register_properties(e: &SignalEmitter<'_>, props: OwnedValue) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn update_property(e: &SignalEmitter<'_>, prop: OwnedValue) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn require_surrounding_text(e: &SignalEmitter<'_>) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn delete_surrounding_text(e: &SignalEmitter<'_>, offset: i32, nchars: u32) -> zbus::Result<()>;
}

/// The string field of an IBusText variant: ("IBusText", a{sv}, s, v).
fn text_of(value: &OwnedValue) -> String {
    let mut current: &Value = value;
    while let Value::Value(inner) = current {
        current = inner;
    }
    match current {
        Value::Structure(s) => match s.fields().get(2) {
            Some(Value::Str(text)) => text.to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// org.freedesktop.IBus.Service on each engine path: the daemon destroys engines through it.
struct ServiceObject {
    path: OwnedObjectPath,
}

#[interface(name = "org.freedesktop.IBus.Service")]
impl ServiceObject {
    async fn destroy(&self, #[zbus(connection)] connection: &Connection) {
        let server = connection.object_server();
        let _ = server.remove::<EngineObject, _>(&self.path).await;
        let _ = server.remove::<ServiceObject, _>(&self.path).await;
    }
}

pub struct Factory {
    engine: String,
    maker: Maker,
    next: AtomicU32,
}

impl Factory {
    pub fn new(engine: &str, maker: Maker) -> Self {
        Self { engine: engine.into(), maker, next: AtomicU32::new(1) }
    }
}

#[interface(name = "org.freedesktop.IBus.Factory")]
impl Factory {
    async fn create_engine(&self, name: String, #[zbus(connection)] connection: &Connection) -> zbus::fdo::Result<OwnedObjectPath> {
        if name != self.engine {
            return Err(zbus::fdo::Error::InvalidArgs(format!("no engine named {name}")));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let path: OwnedObjectPath = ObjectPath::try_from(format!("/org/freedesktop/IBus/Engine/{id}"))
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            .into();
        let waker = Waker { connection: connection.clone(), path: path.clone() };
        let session = (self.maker)(waker);
        let server = connection.object_server();
        server.at(&path, EngineObject { session }).await?;
        server.at(&path, ServiceObject { path: path.clone() }).await?;
        Ok(path)
    }
}
