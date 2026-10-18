//! The nine pages of ui/MainWindow.xaml with the same labels, descriptions and enable conditions.
//! Where GNOME draws something itself or the Linux engine lacks a feature, the row says so and the
//! value is still kept in profile.ini.
use crate::{
    draft::{Draft, PLAIN_KEYS, SHIFT_KEYS},
    models,
    ui::{self, button_row, buttons, choice, enabled, group, number, switch, Page, Severity, Shared},
};
use adw::prelude::*;
use std::{cell::RefCell, path::PathBuf};

const NOT_ON_LINUX: &str = "Linux 版尚未提供加詞浮層，此項目前不生效；設定會保留。";

pub fn all(ui: &Shared) -> Vec<Page> {
    vec![
        page("general", "一般", "go-home-symbolic", general(ui)),
        page("english", "中英混打與切換", "preferences-desktop-locale-symbolic", english(ui)),
        page("keys", "快捷鍵與符號鍵", "input-keyboard-symbolic", keys(ui)),
        page("punctuation", "標點符號", "format-text-plaintext-symbolic", punctuation(ui)),
        page("composition", "組字與提交", "document-edit-symbolic", composition(ui)),
        page("candidates", "選字", "view-list-bullet-symbolic", candidates(ui)),
        page("appearance", "外觀", "preferences-desktop-appearance-symbolic", appearance(ui)),
        page("dictionary", "預測、詞庫與學習", "accessories-dictionary-symbolic", dictionary(ui)),
        page("about", "關於與相容性", "help-about-symbolic", about(ui)),
    ]
}

fn page(id: &'static str, title: &'static str, icon: &'static str, groups: Vec<adw::PreferencesGroup>) -> Page {
    let content = adw::PreferencesPage::builder().title(title).icon_name(icon).build();
    for g in groups {
        content.add(&g);
    }
    Page { id, title, icon, content }
}

thread_local! {
    static PRESETS: RefCell<Option<(adw::ComboRow, Vec<(String, PathBuf)>)>> = const { RefCell::new(None) };
}

const BUILTIN_PRESETS: [&str; 3] = ["選擇操作預設…", "Windows／微軟注音按鍵（預測關閉）", "善解上游操作"];

/// MainWindow.RefreshPresets: the built-in sets, then saved presets under presets/.
pub fn refresh_presets(ui: &Shared) {
    let saved = ui.draft.borrow().saved_presets();
    PRESETS.with_borrow_mut(|slot| {
        let Some((row, list)) = slot.as_mut() else { return };
        let names: Vec<&str> = BUILTIN_PRESETS.iter().copied().chain(saved.iter().map(|(n, _)| n.as_str())).collect();
        *list = saved.clone();
        let _guard = Blocked::new();
        row.set_model(Some(&gtk::StringList::new(&names)));
        row.set_selected(0);
    });
}

thread_local! {
    static BLOCKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
struct Blocked(bool);
impl Blocked {
    fn new() -> Self {
        Self(BLOCKED.replace(true))
    }
}
impl Drop for Blocked {
    fn drop(&mut self) {
        BLOCKED.set(self.0);
    }
}

fn preset_row(ui: &Shared) -> adw::ComboRow {
    let row = adw::ComboRow::builder().title("載入操作預設").build();
    PRESETS.with_borrow_mut(|slot| *slot = Some((row.clone(), vec![])));
    refresh_presets(ui);
    let ui = ui.clone();
    row.connect_selected_notify(move |r| {
        let index = r.selected() as usize;
        if BLOCKED.get() || index == 0 || index as u32 == gtk::INVALID_LIST_POSITION {
            return;
        }
        if index <= 2 {
            ui.result(Severity::Info, |d| d.preset(index == 1).map(str::to_owned));
        } else if let Some((name, file)) = PRESETS.with_borrow(|s| s.as_ref().and_then(|(_, l)| l.get(index - 3).cloned())) {
            ui.result(Severity::Info, |d| d.apply_saved_preset(&file, &name));
        }
        let r = r.clone();
        gtk::glib::idle_add_local_once(move || {
            let _guard = Blocked::new();
            r.set_selected(0);
        });
    });
    row
}

fn ini_filter() -> gtk::gio::ListStore {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("善解操作預設"));
    filter.add_pattern("*.ini");
    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    filters
}

fn import_preset(ui: &Shared) {
    let dialog = gtk::FileDialog::builder().title("匯入操作預設").filters(&ini_filter()).modal(true).build();
    let ui = ui.clone();
    dialog.open(Some(&ui.window.clone()), None::<&gtk::gio::Cancellable>, move |file| {
        let Ok(file) = file else { return };
        let Some(path) = file.path() else { return };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => return ui.show(Severity::Error, &format!("無法讀取操作預設：{error}")),
        };
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        ui.result(Severity::Info, |d| d.import_preset(&text, &stem));
        refresh_presets(&ui);
    });
}

fn export_preset(ui: &Shared) {
    let dialog = gtk::FileDialog::builder().title("匯出目前操作").filters(&ini_filter()).initial_name("我的善解操作.ini").modal(true).build();
    let ui = ui.clone();
    dialog.save(Some(&ui.window.clone()), None::<&gtk::gio::Cancellable>, move |file| {
        let Ok(file) = file else { return };
        let Some(path) = file.path() else { return };
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let text = match ui.draft.borrow().export_preset(&stem) {
            Ok(text) => text,
            Err(error) => return ui.show(Severity::Error, &error.to_string()),
        };
        match std::fs::write(&path, text) {
            Ok(()) => ui.show(Severity::Success, &format!("已匯出到 {}。檔案只包含按鍵與選字操作。", path.display())),
            Err(error) => ui.show(Severity::Error, &format!("無法寫入檔案：{error}")),
        }
    });
}

fn general(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    let presets = group("操作預設", "預設只改按鍵、選字與預測，按儲存後才套用。詞句偏好、詞庫、學習、鍵盤配置與外觀會保留；匯出的檔案也不包含這些內容，可以放心分享。");
    presets.add(&preset_row(ui));
    let (a, b) = (ui.clone(), ui.clone());
    presets.add(&buttons(vec![("匯入操作預設…", Box::new(move || import_preset(&a))), ("匯出目前操作…", Box::new(move || export_preset(&b)))]));

    let input = group("輸入", "");
    input.add(&choice(
        ui,
        "鍵盤配置",
        "Linux 版提供標準注音與倚天注音。",
        "input",
        "keyboard",
        &[("0", "跟隨預設配置（標準注音）"), ("1", "標準注音"), ("2", "倚天注音"), ("3", "漢語拼音"), ("4", "臺灣拼音（通用拼音）")],
    ));
    input.add(&choice(ui, "預設輸入模式", "", "input", "chinese", &[("1", "中文"), ("0", "英文")]));
    input.add(&choice(ui, "預設字元寬度", "", "input", "full_width", &[("1", "全形"), ("0", "半形")]));

    let model = group("語言模型", "線上清單只提供已確認相容的配套；新版本不代表所有情境的準確度都更高。");
    model.add(&choice(ui, "詞句偏好", "", "engine", "ranking", &[("chat", "聊天"), ("formal", "正式")]));
    models::rows(ui, &model);
    vec![presets, input, model]
}

fn english(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    let toggle = group("中英切換", "");
    toggle.add(&switch(ui, "單按 Shift 切換中英", "", "shortcuts", "shift_toggle"));
    let side = choice(ui, "切換用的 Shift 鍵", "", "shortcuts", "shift_side", &[("0", "左右 Shift 都可"), ("1", "僅左 Shift"), ("2", "僅右 Shift")]);
    enabled(ui, &side, |d| d.flag("shortcuts", "shift_toggle"));
    toggle.add(&side);
    toggle.add(&switch(ui, "Ctrl＋空白切換中英", "額外快捷鍵，Windows 預設關閉，保留給 App 的補全等操作。", "shortcuts", "ctrl_space_toggle"));
    toggle.add(&switch(ui, "Caps Lock 輸入英文", "關閉後，Caps Lock 不會讓注音按鍵變成英文。", "input", "caps_lock_english"));

    let mixed = group("中英混打", "注音順序：按鍵排不成合法注音就當英文，組字區先顯示英文按鍵並提示注音，輸入聲調後轉成中文，Enter 保留英文。英文詞表：打出的字母是英文字就當英文，按空白開出原字／首字大寫／全大寫選單，英文留在組字區可接著打中文。兩者並用時，注音順序先判斷，排得成注音但在詞表內的字交給詞表。上游英文模式下 Shift＋字母可開始英文組字段；上游英文詞表僅支援標準注音。倚天可使用 Windows 注音順序判斷；跟隨配置時依實際鍵盤生效。");
    mixed.add(&mixed_mode(ui));
    let inline = switch(ui, "打字時直接顯示英文字母", "只對英文詞表判斷有效：打到至少三個字母且是英文字的開頭時，組字區改顯示英文，不顯示注音。關閉時按空白才跳出英文。", "input", "english_inline");
    enabled(ui, &inline, |d| supports_upstream(d) && d.mixed_mode() >= 2);
    mixed.add(&inline);
    let tolerance = switch(ui, "混打相鄰重複鍵容錯", "只修正 Windows 注音判斷的相鄰重複鍵；上游英文段保持原樣。預設關閉，原始按鍵可由候選選回。", "input", "mixed_tolerance");
    enabled(ui, &tolerance, Draft::is_zhuyin);
    mixed.add(&tolerance);
    mixed.add(&switch(ui, "無法轉換的注音／拼音序列輸出英數", "", "input", "alphanumeric"));
    vec![toggle, mixed]
}

/// SupportsUpstreamEnglish: the upstream English list works with the standard layout only.
fn supports_upstream(d: &Draft) -> bool {
    d.keyboard() < 2
}

fn mixed_mode(ui: &Shared) -> adw::ComboRow {
    const LABELS: [&str; 4] = ["關閉", "注音順序自動判斷（Windows 版）", "英文詞表判斷（上游，僅標準注音）", "兩者並用（僅標準注音）"];
    let row = adw::ComboRow::builder().title("中英混打方式").model(&gtk::StringList::new(&LABELS)).build();
    let shown = row.clone();
    ui.sync(move |d| shown.set_selected(d.mixed_mode()));
    enabled(ui, &row, Draft::is_zhuyin);
    let edit = ui.clone();
    row.connect_selected_notify(move |r| {
        if edit.syncing() {
            return;
        }
        let mode = r.selected();
        // The two upstream choices are disabled for other layouts on Windows.
        if mode >= 2 && !supports_upstream(&edit.draft.borrow()) {
            edit.refresh();
            edit.show(Severity::Info, "上游英文詞表僅支援標準注音；倚天可使用 Windows 注音順序判斷。");
            return;
        }
        edit.edit(|d| d.set_mixed_mode(mode));
    });
    row
}

fn keys(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    let width = group("全半形切換", "");
    width.add(&switch(ui, "Shift＋空白切換全半形", "", "shortcuts", "shift_space_toggle"));

    let symbols = group("符號快捷鍵", "Super 鍵與一般 App 的 Ctrl／Alt 快捷鍵交由系統及 App 處理。");
    symbols.add(&switch(ui, "反白字詞後，快捷鍵加詞", &format!("在可編輯欄位選取漢字，於詞旁確認讀音後加入字典。{NOT_ON_LINUX}"), "shortcuts", "add_word"));
    let shortcut = choice(ui, "加詞快捷鍵", "", "shortcuts", "add_word_shortcut", &[("0", "連按兩次 Ctrl"), ("1", "Ctrl＋Shift＋F12")]);
    let side = choice(ui, "雙按使用的 Ctrl 鍵", "", "shortcuts", "add_word_side", &[("0", "左右均可（兩次須同一側）"), ("1", "僅左 Ctrl"), ("2", "僅右 Ctrl")]);
    let interval = number(ui, "雙按間隔（毫秒）", "", "shortcuts", "add_word_interval", (200, 1000, 50));
    for row in [shortcut.upcast_ref::<gtk::Widget>(), side.upcast_ref(), interval.upcast_ref()] {
        enabled(ui, row, |d| d.flag("shortcuts", "add_word"));
    }
    symbols.add(&shortcut);
    symbols.add(&side);
    symbols.add(&interval);
    symbols.add(&switch(ui, "Ctrl＋標點輸入全形標點", "例如 Ctrl＋逗號輸入 ，；需要 Shift 的符號，仍要一起按 Shift。", "shortcuts", "ctrl_punctuation"));
    symbols.add(&switch(ui, "Ctrl＋反斜線輸入頓號", "善解原有操作；Ctrl 標點開啟時也能輸入頓號。", "shortcuts", "ctrl_backslash"));
    symbols.add(&switch(ui, "反引號＋標點輸入全形標點", "", "shortcuts", "backquote_symbols"));
    symbols.add(&switch(ui, "反引號＋U／B 輸入 Unicode／Big5", "", "shortcuts", "code_input"));
    symbols.add(&switch(ui, "Ctrl＋Alt＋逗號開啟符號鍵盤", "", "shortcuts", "symbol_keyboard"));
    vec![width, symbols]
}

fn punctuation_group(ui: &Shared, title: &str, description: &str, shift: bool) -> adw::PreferencesGroup {
    let g = group(title, description);
    let actions = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(6).valign(gtk::Align::Center).build();
    for (label, on) in [("全部開啟", true), ("全部關閉", false)] {
        let button = gtk::Button::builder().label(label).css_classes(["flat"]).build();
        let ui = ui.clone();
        button.connect_clicked(move |_| ui.edit(|d| d.set_all_punctuation(shift, on)));
        actions.append(&button);
    }
    g.set_header_suffix(Some(&actions));
    let grid = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).homogeneous(true).max_children_per_line(4).min_children_per_line(2).row_spacing(4).column_spacing(12).css_classes(["punctuation"]).build();
    for (key, label) in if shift { SHIFT_KEYS } else { PLAIN_KEYS } {
        let check = gtk::CheckButton::with_label(label);
        let shown = check.clone();
        ui.sync(move |d| shown.set_active(d.flag("punctuation", key)));
        let edit = ui.clone();
        check.connect_toggled(move |c| {
            let on = c.is_active();
            edit.edit(|d| d.set("punctuation", key, on as u32));
        });
        grid.insert(&check, -1);
    }
    let card = gtk::Frame::builder().child(&grid).css_classes(["card"]).build();
    grid.set_margin_top(8);
    grid.set_margin_bottom(8);
    grid.set_margin_start(12);
    grid.set_margin_end(12);
    g.add(&card);
    g
}

fn punctuation(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    vec![
        punctuation_group(ui, "Shift＋符號直接輸入中文標點", "善解／Apple 式操作，每個符號可分別開關。關閉的按鍵輸出原本的英文符號。Windows 預設全部關閉。", true),
        punctuation_group(ui, "直接按鍵輸入中文標點", "不按 Ctrl，直接按鍵輸入標點；每個按鍵分別設定，例如只開「」而不讓 \\ 變成頓號。注音鍵（如倚天的 ' 與 =）不受影響。", false),
    ]
}

fn composition(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    let tones = group("", "");
    tones.add(&switch(ui, "沒有組字時，聲調鍵輸入聲調符號", "善解特有操作：例如標準鍵盤 3 輸入 ˇ，留在組字區，Enter 才提交。關閉時輸入原本的數字。", "input", "standalone_tones"));

    let escape = group("Esc", "");
    escape.add(&choice(ui, "未開候選時的 Esc", "候選或已進入的預測列仍先關閉／退出，不會因這個選項立即刪除整句。", "input", "escape", &[("staged", "先清未完成的讀音，再清整句"), ("clear", "一次清除全部未提交組字"), ("ignore", "保留組字")]));

    let enter = group("Enter", "");
    enter.add(&switch(ui, "Enter 只提交游標前的字", "游標在組字中間時，Enter 只送出游標前（選字後含所選的字）的部分，後面的字繼續留在組字區可再選字。關閉時一律送出整句。", "candidates", "enter_to_cursor"));

    let space = group("空白鍵", "");
    let zhuyin = switch(ui, "空白鍵輸入注音符號", "開啟沿用注音符號輸入。關閉時，無法轉換的按鍵照原樣輸出，例如 1＋空白得到數字 1 和空格；第一聲仍可轉換。字詞候選第一項提供注音。", "input", "space_inputs_zhuyin");
    enabled(ui, &zhuyin, Draft::is_zhuyin);
    space.add(&zhuyin);
    space.add(&switch(ui, "空白鍵提交組字", "預設關閉：直接在組字中插入空格，仍可繼續編輯。注音第一聲與拼音轉換依原有規則處理。", "input", "space_commits"));
    let after = switch(ui, "提交後再補一個空格", "只在空白鍵用來提交組字時生效。", "input", "space_after_commit");
    enabled(ui, &after, |d| d.flag("input", "space_commits"));
    space.add(&after);
    space.add(&switch(ui, "拼音按一次空白就轉換", "關閉時，第一次空白分隔音節，Enter 或連按兩次空白轉換。聲調鍵 1–5 皆可直接轉換。", "input", "pinyin_space_converts"));
    vec![tones, escape, enter, space]
}

fn candidates(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    let operation = group("選字操作", "");
    let help = operation.clone();
    ui.sync(move |d| help.set_description(Some(&d.candidate_help())));
    operation.add(&choice(ui, "方向鍵操作", "", "profile", "interaction", &[("windows", "Windows／微軟注音 ↑↓"), ("shanjie", "善解上游 ←→")]));
    operation.add(&choice(ui, "選字位置", "", "candidates", "anchor", &[("right", "游標右側的字／詞"), ("left", "游標左側的字／詞")]));
    operation.add(&switch(ui, "候選中 Enter 選字並提交整句", "關閉時只確認候選，繼續留在組字區。", "candidates", "enter_commits"));
    operation.add(&switch(ui, "未開候選時，↑ 直接換下一個候選", "", "candidates", "up_cycles"));
    operation.add(&switch(ui, "候選中 Backspace 同時刪除前一個字", "關閉時只收回候選，保留組字。", "candidates", "backspace_deletes"));

    let space = group("空白鍵操作候選", "");
    space.add(&switch(ui, "空白鍵操作候選字", "預設關閉：用 ↓ 開啟選字，空白不開啟或翻動候選。開啟後依下方設定操作；候選操作優先於空白提交。", "candidates", "space_operates"));
    let open = choice(ui, "完整組字後開啟選字", "", "candidates", "open", &[("down", "↓ 下鍵"), ("space", "空白鍵"), ("both", "↓ 或空白鍵")]);
    enabled(ui, &open, |d| d.flag("candidates", "space_operates"));
    space.add(&open);
    let pages = switch(ui, "空白換下一頁候選", "", "candidates", "space_pages");
    enabled(ui, &pages, |d| d.flag("candidates", "space_operates"));
    space.add(&pages);
    let wrap = switch(ui, "最後一頁再按空白回到第一頁", "", "candidates", "wrap_pages");
    enabled(ui, &wrap, |d| d.flag("candidates", "space_operates") && d.flag("candidates", "space_pages"));
    space.add(&wrap);

    let charset = group("字元集", "");
    charset.add(&switch(ui, "候選只顯示 Big5 字元集", "同時套用到轉換候選、即時預測與聯想詞；已組好的文字不會被轉換。", "candidates", "big5"));
    vec![operation, space, charset]
}

fn appearance(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    let window = group("候選視窗", "每頁最多九項；直式一列一個候選，與微軟注音相同。候選框的外觀與字型由 GNOME 繪製。");
    window.add(&choice(ui, "候選排列", "", "candidates", "layout", &[("horizontal", "橫向"), ("vertical", "直式（微軟注音）")]));
    window.add(&number(ui, "候選文字大小", "GNOME 依系統字型決定候選文字大小，此值目前不生效；設定會保留。", "appearance", "font_size", (14, 32, 1)));

    let theme = group("主題與提示", "");
    theme.add(&choice(ui, "介面與選字框主題", "套用到設定視窗；候選框跟隨 GNOME 外觀。", "appearance", "theme", &[("0", "跟隨系統"), ("1", "淺色"), ("2", "深色")]));
    theme.add(&switch(ui, "切換中英時顯示游標旁提示", "GNOME 在頂端列顯示中／英狀態；游標旁提示目前不生效，設定會保留。", "appearance", "mode_indicator"));
    vec![window, theme]
}

fn clear_learning(ui: &Shared) {
    let dialog = adw::AlertDialog::new(Some("清除選字學習？"), Some("會清除本帳號已儲存的選字偏好，個人詞庫與設定會保留。"));
    dialog.add_responses(&[("cancel", "取消"), ("clear", "清除")]);
    dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let ui = ui.clone();
    dialog.choose(Some(&ui.window.clone()), None::<&gtk::gio::Cancellable>, move |response| {
        if response != "clear" {
            return;
        }
        let folder = ui.draft.borrow().folder().to_owned();
        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = sender.send_blocking(shanjie_linux::store::clear_learning(&folder).map_err(|e| e.to_string()));
        });
        gtk::glib::spawn_future_local(async move {
            match receiver.recv().await {
                Ok(Ok(())) => ui.show(Severity::Success, "已清除選字學習。"),
                Ok(Err(error)) => ui.show(Severity::Error, &format!("無法清除學習：{error}")),
                Err(_) => {}
            }
        });
    });
}

fn dictionary_editor(ui: &Shared) -> gtk::Widget {
    let buffer = gtk::TextBuffer::new(None);
    let view = gtk::TextView::builder().buffer(&buffer).monospace(true).wrap_mode(gtk::WrapMode::WordChar).top_margin(8).bottom_margin(8).left_margin(10).right_margin(10).build();
    let scroll = gtk::ScrolledWindow::builder().child(&view).min_content_height(160).max_content_height(300).propagate_natural_height(true).build();
    let shown = buffer.clone();
    ui.sync(move |d| {
        if shown.text(&shown.start_iter(), &shown.end_iter(), false) != d.dictionary {
            shown.set_text(&d.dictionary);
        }
    });
    let edit = ui.clone();
    buffer.connect_changed(move |b| {
        let text = b.text(&b.start_iter(), &b.end_iter(), false).to_string();
        edit.edit(|d| d.set_dictionary(text));
    });
    gtk::Frame::builder().child(&scroll).css_classes(["card"]).build().upcast()
}

fn dictionary(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    let options = group("", "");
    options.add(&switch(ui, "未完成讀音的即時預測", "第一個注音鍵就提示可能的完整詞。Tab 進入，數字／Enter 選詞；Shift＋Tab 或 Esc 退出。Windows 預設關閉。", "engine", "prediction"));
    options.add(&switch(ui, "選字後的聯想詞", "數字或滑鼠選字後提示尚未輸入的後續詞，以 Shift＋數字或滑鼠選取。Windows 預設關閉。", "input", "associations"));
    options.add(&switch(ui, "使用游標前文改善選字", "只使用游標前的少量文字作為本機選字與學習的前文，不傳送到網路。", "input", "context"));
    options.add(&switch(ui, "避免敏感詞排在第一名", "套用上游的特定詞降權表；詞仍可選取，並非刪除詞庫。", "engine", "demote"));
    options.add(&switch(ui, "學習我的選字偏好", "", "input", "learning"));
    let forget = switch(ui, "Ctrl＋Shift＋Backspace 忘記目前候選的學習", "學習開啟且有反白候選時生效；不刪除個人詞庫。", "shortcuts", "forget_candidate");
    enabled(ui, &forget, |d| d.flag("input", "learning"));
    options.add(&forget);
    let clear = ui.clone();
    let row = button_row("清除已儲存的選字學習", "user-trash-symbolic", move || clear_learning(&clear));
    row.add_css_class("destructive-action");
    options.add(&row);

    let personal = group("個人詞庫", "每行填「注音 字詞」，例如：ㄕㄢˋ-ㄐㄧㄝˇ 善解。可在第三欄指定非正數分數，預設 -0.01。儲存後先提交組字再載入。");
    personal.add(&dictionary_editor(ui));
    vec![options, personal]
}

fn link(ui: &Shared, title: &str, uri: &'static str) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).subtitle(uri).activatable(true).build();
    row.add_suffix(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
    let window = ui.window.clone();
    row.connect_activated(move |_| gtk::UriLauncher::new(uri).launch(Some(&window), None::<&gtk::gio::Cancellable>, |_| {}));
    row
}

fn about(ui: &Shared) -> Vec<adw::PreferencesGroup> {
    let about = group("善解 Linux", "GTK 4 設定 · GNOME 原生 IBus 候選 · 本機注音引擎");
    about.add(&ui::note("輸入內容、詞庫、學習及設定都留在電腦上。操作預設調整按鍵與候選；鍵盤配置決定標準／倚天或拼音。"));

    let microsoft = group("與微軟注音的差異", "Windows 操作預設採微軟注音的常用鍵位，關閉即時預測、選字後聯想及善解的額外標點／聲調操作。載入預設後可逐項調整，再按儲存套用。\n候選順序仍由善解的詞庫、模型與學習決定。微軟的手寫／部首辨識、雲端功能、舊版引擎與私有詞庫格式不包含在善解中。");
    microsoft.add(&link(ui, "微軟注音官方操作說明", "https://support.microsoft.com/en-us/windows/hardware/input-devices/microsoft-traditional-chinese-ime"));
    microsoft.add(&link(ui, "上游 model-v6 與詞類表說明", "https://github.com/Nanako0129/shanjie/releases/tag/model-v6"));

    let gnome = group("GNOME 上的差異", "候選框由 GNOME 繪製，外觀、字型大小與主題跟隨系統；中／英狀態顯示在頂端列，沒有游標旁提示。尚未提供反白加詞（連按兩次 Ctrl／Ctrl＋Shift＋F12）。翻頁可用空白、Page Up 與 Page Down。設定檔與詞庫位於 ~/.local/share/shanjie。");
    vec![about, microsoft, gnome]
}
