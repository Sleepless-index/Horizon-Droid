//! Native account panel; authentication is shared with the desktop launcher.
use super::pages::{Action, button};
use crate::account::{self, Account, Accounts, Player, Secrets, SignIn, SignInState};
use horizon_client::ui_theme::{Face, Icon, Ui, color};

fn login_surface() -> u32 {
    horizon_client::ui_theme::active_theme().surface
}
fn login_surface_high() -> u32 {
    horizon_client::ui_theme::active_theme().surface_bright
}
fn login_border() -> u32 {
    horizon_client::ui_theme::active_theme().border
}
fn login_text() -> u32 {
    horizon_client::ui_theme::active_theme().text_primary
}
fn login_muted() -> u32 {
    horizon_client::ui_theme::active_theme().text_muted
}
fn login_accent() -> u32 {
    horizon_client::ui_theme::active_theme().accent
}
fn login_accent_soft() -> u32 {
    horizon_client::ui_theme::active_theme().accent_soft
}

pub(super) const ROWS_PER_PAGE: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Account(usize),
    Offline(usize),
}

pub(super) struct Login {
    pub choosing: bool,
    pub offline: bool,
    pub saved_accounts: Vec<Account>,
    pub offline_names: Vec<String>,
    pub account_offset: usize,
    pub menu_open: bool,
    pub new_offline: String,
    selected_key: Option<String>,
    pending_remove: Option<(bool, usize)>,
    pub ely: bool,
    pub username: String,
    pub password: String,
    pub otp: String,
    pub visible: bool,
    pub signed_in: bool,
    pub profile: Option<String>,
    task: Option<SignIn>,
    auto_open_device: bool,
    state: Option<SignInState>,
    message: String,
}
impl Login {
    pub fn new() -> Self {
        let mut login = Self {
            choosing: false,
            offline: false,
            saved_accounts: vec![],
            offline_names: vec![],
            account_offset: 0,
            menu_open: false,
            new_offline: String::new(),
            selected_key: None,
            pending_remove: None,
            ely: false,
            username: String::new(),
            password: String::new(),
            otp: String::new(),
            visible: false,
            signed_in: false,
            profile: None,
            task: None,
            auto_open_device: false,
            state: None,
            message: String::new(),
        };
        login.refresh_status();
        login
    }
    pub(super) fn refresh_status(&mut self) {
        let accounts = Accounts::load();
        let account = accounts.selected().cloned();
        self.selected_key = account.as_ref().map(|a| a.key());
        self.saved_accounts = accounts.accounts;
        self.offline_names = accounts.offline;
        self.profile = account.as_ref().map(|a| a.name.clone());
        self.signed_in = account.is_some_and(|a| {
            account::store()
                .get(&a.key())
                .ok()
                .flatten()
                .is_some_and(|s| serde_json::from_str::<Secrets>(&s).is_ok())
        });
    }

    /// Keeps the offline name already playing in the saved list.
    pub(super) fn seed_offline(&mut self, name: &str) {
        let name = name.trim();
        let mut accounts = Accounts::load();
        if account::valid_offline_name(name)
            && !accounts.offline.iter().any(|n| n.eq_ignore_ascii_case(name))
        {
            accounts.offline.push(name.to_string());
            let _ = accounts.save();
        }
    }

    pub(super) fn open_list(&mut self) {
        self.refresh_status();
        self.visible = true;
        self.choosing = true;
        self.offline = false;
        self.ely = false;
        self.menu_open = false;
        self.pending_remove = None;
        self.new_offline.clear();
        self.account_offset = 0;
        self.message.clear();
    }

    pub(super) fn set_message(&mut self, message: String) {
        self.message = message;
    }

    pub(super) fn row_count(&self) -> usize {
        self.saved_accounts.len() + self.offline_names.len()
    }

    fn rows(&self) -> Vec<Row> {
        (0..self.saved_accounts.len())
            .map(Row::Account)
            .chain((0..self.offline_names.len()).map(Row::Offline))
            .collect()
    }

    /// The second tap on the same remove button confirms it.
    pub(super) fn confirm_remove(&mut self, offline: bool, index: usize) -> bool {
        if self.pending_remove == Some((offline, index)) {
            self.pending_remove = None;
            return true;
        }
        self.pending_remove = Some((offline, index));
        false
    }

    pub(super) fn clear_pending(&mut self) {
        self.menu_open = false;
        self.pending_remove = None;
    }

    pub(super) fn remove_account(&mut self, index: usize) {
        let Some(account) = self.saved_accounts.get(index).cloned() else {
            return;
        };
        let mut accounts = Accounts::load();
        self.message = match accounts
            .remove_when_idle(account::store(), &account.key())
            .and_then(|_| accounts.save())
        {
            Ok(()) => format!("Removed {}", account.name),
            Err(e) => e,
        };
        self.refresh_status();
    }

    /// Forgets an offline name. Returns the offline name that plays now when it was the one
    /// that played (empty: none left).
    pub(super) fn remove_offline(&mut self, index: usize, active: &str) -> Option<String> {
        let name = self.offline_names.get(index).cloned()?;
        let mut accounts = Accounts::load();
        accounts.offline.retain(|n| !n.eq_ignore_ascii_case(&name));
        accounts.looks.remove(&name.to_ascii_lowercase());
        let was_playing = accounts.selected().is_none() && active.eq_ignore_ascii_case(&name);
        if accounts
            .offline_selected
            .as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(&name))
        {
            accounts.offline_selected = None;
        }
        let next = was_playing.then(|| accounts.offline.first().cloned().unwrap_or_default());
        if let Some(next) = next.as_ref().filter(|n| !n.is_empty()) {
            accounts.offline_selected = Some(next.clone());
        }
        self.message = match accounts.save() {
            Ok(()) => format!("Removed {name}"),
            Err(e) => e,
        };
        self.refresh_status();
        next
    }

    /// Plays as a kept offline name; returns it.
    pub(super) fn select_offline(&mut self, index: usize) -> Option<String> {
        let name = self.offline_names.get(index).cloned()?;
        let mut accounts = Accounts::load();
        accounts.selected = None;
        accounts.offline_selected = Some(name.clone());
        if let Err(e) = accounts.save() {
            self.message = e;
        }
        self.refresh_status();
        Some(name)
    }

    /// Keeps the typed offline name and plays as it.
    pub(super) fn add_offline(&mut self) -> Result<String, String> {
        let name = self.new_offline.trim().to_string();
        let mut accounts = Accounts::load();
        accounts.play_as_with(account::store(), Player::Offline(name), true)?;
        accounts.save()?;
        let name = accounts.offline_selected.clone().unwrap_or_default();
        self.new_offline.clear();
        self.refresh_status();
        Ok(name)
    }

    pub fn pending(&self) -> bool {
        self.task.is_some()
    }
    pub fn hide(&mut self) {
        self.password.clear();
        self.otp.clear();
        self.visible = false;
        self.menu_open = false;
        self.pending_remove = None;
    }
    pub fn close(&mut self) {
        self.password.clear();
        self.otp.clear();
        self.auto_open_device = false;
        if let Some(task) = self.task.take() {
            task.cancel();
        }
        self.state = None;
        self.visible = false;
    }
    pub fn start(&mut self, id: &str) {
        self.start_device(id);
        self.auto_open_device = true;
    }
    pub fn start_device(&mut self, id: &str) {
        self.close();
        self.ely = false;
        self.choosing = false;
        self.visible = true;
        let id = id.trim();
        if !account::legacy_client(id)
            && (id.len() != 36
                || !id.chars().enumerate().all(|(i, c)| {
                    if [8, 13, 18, 23].contains(&i) {
                        c == '-'
                    } else {
                        c.is_ascii_hexdigit()
                    }
                }))
        {
            self.message =
                "Register our Microsoft public client application first, then enter its client ID."
                    .into();
            return;
        }
        self.message.clear();
        self.task = Some(SignIn::start_with_client_id(Some(id.into())));
        self.state = Some(SignInState::Starting);
    }
    /// Reports completion so views can switch their provider-scoped selection,
    /// including when the account panel was closed during browser sign-in.
    pub fn poll(&mut self) -> bool {
        let mut completed = false;
        if let Some(task) = &self.task {
            self.state = Some(task.state());
            let done = match &self.state {
                Some(SignInState::Done(result)) => Some(result.clone()),
                _ => None,
            };
            if let Some(result) = done {
                self.message = match &result {
                    Ok(a) => format!("Signed in as {}", a.name),
                    Err(e) => e.clone(),
                };
                self.task = None;
                self.refresh_status();
                if result.is_ok() {
                    self.choosing = true;
                    self.ely = false;
                    self.offline = false;
                    self.account_offset = 0;
                }
                completed = true;
            }
        }
        if self.auto_open_device && matches!(self.state, Some(SignInState::Code(_))) {
            self.auto_open_device = false;
            self.open_link();
        }
        completed
    }
    pub fn start_ely(&mut self) {
        if let Some(task) = self.task.take() {
            task.cancel();
        }
        self.message.clear();
        self.task = Some(SignIn::start_ely(
            self.username.clone(),
            std::mem::take(&mut self.password),
            std::mem::take(&mut self.otp),
        ));
        self.state = Some(SignInState::Starting);
    }
    pub fn field(&self, name: &str) -> Option<&str> {
        match name {
            "ely_username" => Some(&self.username),
            "ely_password" => Some(&self.password),
            "ely_otp" => Some(&self.otp),
            "offline_name" => Some(&self.new_offline),
            _ => None,
        }
    }
    pub fn edit(&mut self, field: Option<&str>, output: &horizon_client::osk::Output) -> bool {
        let value = match field {
            Some("ely_username") => &mut self.username,
            Some("ely_password") => &mut self.password,
            Some("ely_otp") => &mut self.otp,
            Some("offline_name") => &mut self.new_offline,
            _ => return false,
        };
        match output {
            horizon_client::osk::Output::Text(text) if value.len() + text.len() <= 1024 => {
                value.push_str(text);
            }
            horizon_client::osk::Output::Backspace => {
                value.pop();
            }
            _ => {}
        }
        true
    }
    pub fn copy_code(&mut self) {
        if let Some(SignInState::Code(code)) = &self.state {
            self.message = if crate::platform::copy_login_code(&code.user_code) {
                "Code copied. Paste it on the Microsoft page.".into()
            } else {
                "Could not copy the code. Try again.".into()
            };
        }
    }
    pub fn open_link(&mut self) {
        if let Some(SignInState::Code(code)) = &self.state {
            let mut url = url::Url::parse(&code.verification_uri).ok();
            if let Some(uri) = &mut url {
                uri.query_pairs_mut().append_pair("otc", &code.user_code);
            }
            let url = url
                .map(String::from)
                .unwrap_or_else(|| code.verification_uri.clone());
            self.copy_code();
            if let Err(e) = crate::platform::open_login_url(&url) {
                self.message = e;
            }
        }
    }
    pub fn review_app(&mut self) {
        if let Err(e) = crate::platform::open_login_url("https://aka.ms/mce-reviewappid") {
            self.message = e;
        }
    }
    pub fn session(id: &str) -> Result<Option<(Account, Secrets)>, String> {
        account::session::selected_session(id.trim())
    }
    #[allow(clippy::too_many_arguments)]
    fn list_row(
        &self,
        ui: &mut Ui,
        targets: &mut Vec<([f32; 4], Action)>,
        rect: [f32; 4],
        name: &str,
        detail: &str,
        current: bool,
        select: Action,
        remove: Action,
        confirming: bool,
    ) {
        let theme = horizon_client::ui_theme::active_theme();
        let [x, y, w, h] = rect;
        ui.rounded(
            x,
            y,
            w,
            h,
            12.0,
            if current {
                theme.surface_selected
            } else {
                login_surface()
            },
        );
        ui.stroke_rounded(
            x,
            y,
            w,
            h,
            12.0,
            if current { 1.4 } else { 1.0 },
            if current {
                login_accent()
            } else {
                login_border()
            },
        );
        let avatar = (h - 12.0).clamp(28.0, 40.0);
        ui.avatar_head_2d(x + 10.0, y + (h - avatar) * 0.5, avatar, name);

        let trash_w = if confirming { 84.0 } else { 36.0 };
        let trash = [
            x + w - trash_w - 8.0,
            y + (h - 32.0) * 0.5,
            trash_w,
            32.0,
        ];
        if confirming {
            ui.rounded(trash[0], trash[1], trash[2], trash[3], 9.0, theme.danger);
            ui.centered_fit(
                Face::SemiBold,
                11.5,
                trash,
                theme.on_accent,
                "Remove?",
            );
        } else {
            ui.icon(
                Icon::Trash,
                trash[0] + 8.0,
                trash[1] + 6.0,
                20.0,
                login_muted(),
            );
        }
        targets.push((trash, remove));

        let check_w = if current { 28.0 } else { 0.0 };
        if current {
            ui.icon(
                Icon::Check,
                trash[0] - check_w,
                y + (h - 18.0) * 0.5,
                18.0,
                login_accent(),
            );
        }
        let tx = x + avatar + 22.0;
        let text_w = (trash[0] - check_w - tx - 6.0).max(60.0);
        let fitted = ui.fit_text(Face::SemiBold, 14.0, name, text_w);
        ui.text(Face::SemiBold, 14.0, tx, y + h * 0.5 - 17.0, login_text(), &fitted);
        let detail = if current {
            format!("{detail} · Playing")
        } else {
            detail.to_owned()
        };
        ui.text(
            Face::Regular,
            11.5,
            tx,
            y + h * 0.5 + 3.0,
            if current {
                login_accent()
            } else {
                login_muted()
            },
            &detail,
        );
        targets.push(([x, y, w, h], select));
    }

    pub fn draw(
        &self,
        ui: &mut Ui,
        targets: &mut Vec<([f32; 4], Action)>,
        offline_name: &str,
        editing: Option<&str>,
        modal_anim: f32,
    ) {
        targets.clear();
        let modal_t = modal_anim.clamp(0.2, 1.0);
        let scale_in = 0.95 + 0.05 * modal_t;
        let w = (ui.w - 24.0).min(620.0) * scale_in;
        let h = (ui.h - 20.0).min(600.0) * scale_in;
        let x = (ui.w - w) * 0.5;
        let mut y = (ui.h - h) * 0.5 + (1.0 - modal_t) * 14.0;
        if editing.is_some()
            && let Some(bottom_px) = crate::native_ime::viewport_bottom()
        {
            let ime_top_dp = bottom_px as f32 / ui.density;
            if ime_top_dp < ui.h - 16.0 {
                y = y.min((ime_top_dp - h - 8.0).max(6.0));
            }
        }
        let theme = horizon_client::ui_theme::active_theme();
        ui.fill(
            0.0,
            0.0,
            ui.w,
            ui.h,
            color::with_alpha(theme.bg_solid, 0.65 * modal_t),
        );
        ui.rounded(
            x + 2.0,
            y + 8.0,
            w,
            h,
            12.0,
            color::hex(0x000000, (90.0 * modal_t) as u8),
        );
        ui.rounded(x, y, w, h, 12.0, theme.bg_solid);
        ui.border(x, y, w, h, 12.0, login_border());

        let list_mode = self.choosing;
        let plus = [x + w - 100.0, y + 16.0, 38.0, 38.0];
        let menu_w = 244.0_f32.min(w - 16.0);
        let menu_item_h = 52.0;
        let menu_items = [
            (
                "Microsoft",
                "Xbox and Minecraft",
                "M",
                Action::MicrosoftAccount,
            ),
            ("Ely.by", "Alternative auth server", "E", Action::ElySignIn),
            (
                "Offline",
                "Play without signing in",
                "O",
                Action::OfflineAccount,
            ),
        ];
        let menu = [
            (plus[0] + plus[2] - menu_w).max(x + 8.0),
            plus[1] + plus[3] + 6.0,
            menu_w,
            menu_item_h * menu_items.len() as f32 + 8.0,
        ];
        if list_mode && self.menu_open {
            for (i, (_, _, _, action)) in menu_items.iter().enumerate() {
                targets.push((
                    [
                        menu[0] + 4.0,
                        menu[1] + 4.0 + i as f32 * menu_item_h,
                        menu[2] - 8.0,
                        menu_item_h,
                    ],
                    *action,
                ));
            }
            targets.push(([0.0, 0.0, ui.w, ui.h], Action::AddAccount));
        }

        let (title, subtitle): (&str, String) = if list_mode {
            let line = if !self.message.is_empty() {
                self.message.clone()
            } else {
                match self.row_count() {
                    0 => "Tap + to add an account".into(),
                    1 => "1 saved account".into(),
                    n => format!("{n} saved accounts"),
                }
            };
            ("Accounts", line)
        } else if self.ely {
            (
                "Ely.by account",
                "Credentials go only to authserver.ely.by".into(),
            )
        } else if self.offline {
            (
                "Offline account",
                "A local name for single-player and LAN".into(),
            )
        } else {
            (
                "Microsoft account",
                "Xbox and Minecraft profile".into(),
            )
        };
        if !list_mode {
            let back = [x + 16.0, y + 16.0, 38.0, 38.0];
            ui.rounded(
                back[0],
                back[1],
                back[2],
                back[3],
                12.0,
                login_surface_high(),
            );
            ui.icon(
                Icon::ArrowLeft,
                back[0] + 10.0,
                back[1] + 10.0,
                18.0,
                login_text(),
            );
            targets.push((back, Action::Account));
        }
        let close = [x + w - 54.0, y + 16.0, 38.0, 38.0];
        ui.rounded(
            close[0],
            close[1],
            close[2],
            close[3],
            12.0,
            login_surface_high(),
        );
        ui.icon(
            Icon::Close,
            close[0] + 11.0,
            close[1] + 11.0,
            16.0,
            login_text(),
        );
        targets.push((close, Action::CloseLogin));
        if list_mode {
            ui.rounded(plus[0], plus[1], plus[2], plus[3], 12.0, login_accent());
            ui.icon(
                Icon::Plus,
                plus[0] + 9.0,
                plus[1] + 9.0,
                20.0,
                theme.on_accent,
            );
            targets.push((plus, Action::AddAccount));
        }
        let text_x = if list_mode { x + 22.0 } else { x + 64.0 };
        let title_right = if list_mode { x + w - 110.0 } else { x + w - 70.0 };
        ui.text(Face::SemiBold, 20.0, text_x, y + 14.0, login_text(), title);
        let fitted = ui.fit_text(
            Face::Regular,
            12.5,
            &subtitle,
            (title_right - text_x).max(70.0),
        );
        ui.text(
            Face::Regular,
            12.5,
            text_x,
            y + 42.0,
            login_muted(),
            &fitted,
        );
        ui.rounded(x + 20.0, y + 72.0, w - 40.0, 1.0, 0.5, login_border());

        let content_y = y + 88.0;
        let content_h = (y + h - 20.0 - content_y).max(60.0);
        let footer_y = y + h - 58.0;
        let gap = 8.0;
        let field_h = ((content_h - 116.0) / 3.0 - gap).clamp(42.0, 56.0);

        if self.choosing {
            let rows = self.rows();
            let row_gap = 6.0;
            let avail = footer_y - content_y - 8.0;
            let row_h = ((avail - row_gap * (ROWS_PER_PAGE as f32 - 1.0)) / ROWS_PER_PAGE as f32)
                .clamp(36.0, 54.0);
            let pages = rows.len().div_ceil(ROWS_PER_PAGE).max(1);
            let offset = self
                .account_offset
                .min((pages - 1) * ROWS_PER_PAGE);
            if rows.is_empty() {
                let card = [x + 20.0, content_y, w - 40.0, 64.0];
                ui.rounded(card[0], card[1], card[2], card[3], 12.0, login_surface());
                ui.border(card[0], card[1], card[2], card[3], 12.0, login_border());
                ui.text(
                    Face::SemiBold,
                    13.0,
                    card[0] + 16.0,
                    card[1] + 14.0,
                    login_text(),
                    "No saved accounts yet",
                );
                ui.text(
                    Face::Regular,
                    11.5,
                    card[0] + 16.0,
                    card[1] + 36.0,
                    login_muted(),
                    "Tap + to sign in or add an offline name.",
                );
            }
            for (n, row) in rows.iter().enumerate().skip(offset).take(ROWS_PER_PAGE) {
                let ry = content_y + (n - offset) as f32 * (row_h + row_gap);
                let rect = [x + 20.0, ry, w - 40.0, row_h];
                match *row {
                    Row::Account(i) => {
                        let account = &self.saved_accounts[i];
                        let detail = if account.provider == account::Provider::Ely {
                            "Ely.by"
                        } else {
                            "Microsoft"
                        };
                        let current = self.selected_key.as_deref() == Some(account.key().as_str());
                        self.list_row(
                            ui,
                            targets,
                            rect,
                            &account.name,
                            detail,
                            current,
                            Action::SelectAccount(i),
                            Action::RemoveAccount(i),
                            self.pending_remove == Some((false, i)),
                        );
                    }
                    Row::Offline(i) => {
                        let name = &self.offline_names[i];
                        let current = self.selected_key.is_none()
                            && name.eq_ignore_ascii_case(offline_name.trim());
                        self.list_row(
                            ui,
                            targets,
                            rect,
                            name,
                            "Offline",
                            current,
                            Action::SelectOfflineAt(i),
                            Action::RemoveOffline(i),
                            self.pending_remove == Some((true, i)),
                        );
                    }
                }
            }
            let mut done_x = x + 20.0;
            if pages > 1 {
                let nav = 36.0;
                button(
                    ui,
                    targets,
                    [x + 20.0, footer_y, nav, 38.0],
                    "<",
                    Action::SavedPage(false),
                    false,
                );
                let label = format!("{}/{}", offset / ROWS_PER_PAGE + 1, pages);
                let label_w = 44.0;
                ui.centered_fit(
                    Face::Medium,
                    12.0,
                    [x + 20.0 + nav + 4.0, footer_y, label_w, 38.0],
                    login_muted(),
                    &label,
                );
                button(
                    ui,
                    targets,
                    [x + 20.0 + nav + 8.0 + label_w, footer_y, nav, 38.0],
                    ">",
                    Action::SavedPage(true),
                    false,
                );
                done_x = x + 20.0 + 2.0 * nav + label_w + 20.0;
            }
            button(
                ui,
                targets,
                [done_x, footer_y, x + w - 20.0 - done_x, 38.0],
                "Done",
                Action::CloseLogin,
                true,
            );

            if self.menu_open {
                ui.rounded(
                    menu[0] + 2.0,
                    menu[1] + 6.0,
                    menu[2],
                    menu[3],
                    14.0,
                    color::hex(0x000000, 110),
                );
                ui.rounded(menu[0], menu[1], menu[2], menu[3], 14.0, login_surface_high());
                ui.border(menu[0], menu[1], menu[2], menu[3], 14.0, login_border());
                for (i, (label, note, mark, _)) in menu_items.iter().enumerate() {
                    let iy = menu[1] + 4.0 + i as f32 * menu_item_h;
                    ui.rounded(menu[0] + 12.0, iy + 9.0, 34.0, 34.0, 11.0, login_accent_soft());
                    ui.centered_fit(
                        Face::SemiBold,
                        15.0,
                        [menu[0] + 12.0, iy + 9.0, 34.0, 34.0],
                        login_accent(),
                        mark,
                    );
                    ui.text(
                        Face::SemiBold,
                        14.0,
                        menu[0] + 58.0,
                        iy + 8.0,
                        login_text(),
                        label,
                    );
                    let note = ui.fit_text(Face::Regular, 11.0, note, menu[2] - 70.0);
                    ui.text(
                        Face::Regular,
                        11.0,
                        menu[0] + 58.0,
                        iy + 28.0,
                        login_muted(),
                        &note,
                    );
                }
            }
            return;
        }

        if self.ely {
            for (i, (name, label, value, secret, placeholder)) in [
                (
                    "ely_username",
                    "Username or email",
                    self.username.as_str(),
                    false,
                    "Username or email",
                ),
                (
                    "ely_password",
                    "Password",
                    self.password.as_str(),
                    true,
                    "Password",
                ),
                (
                    "ely_otp",
                    "Two-factor code",
                    self.otp.as_str(),
                    true,
                    "Optional",
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let fy = content_y + i as f32 * (field_h + gap);
                let display = if secret {
                    "*".repeat(value.chars().count().min(40))
                } else {
                    value.to_owned()
                };
                let focused = editing == Some(name);
                ui.text_field(
                    x + 20.0,
                    fy,
                    w - 40.0,
                    field_h,
                    label,
                    &display,
                    placeholder,
                    focused,
                );
                targets.push(([x + 20.0, fy, w - 40.0, field_h], Action::Field(name)));
            }
            if editing.is_some_and(|field| field.starts_with("ely_")) {
                button(
                    ui,
                    targets,
                    [x + 20.0, footer_y, w - 40.0, 42.0],
                    "Done editing",
                    Action::FinishNameEdit,
                    true,
                );
                return;
            }
            let note_y = content_y + 3.0 * (field_h + gap) + 4.0;
            if !self.message.is_empty() {
                paragraph(
                    ui,
                    x + 20.0,
                    note_y,
                    w - 40.0,
                    &self.message,
                    login_text(),
                    2,
                );
            }
            button(
                ui,
                targets,
                [x + 20.0, footer_y, w - 40.0, 42.0],
                if self.pending() {
                    "Signing in..."
                } else {
                    "Sign in to Ely.by"
                },
                Action::SubmitEly,
                true,
            );
            button(
                ui,
                targets,
                [x + 20.0, footer_y - 48.0, w - 40.0, 36.0],
                "Back to accounts",
                Action::Account,
                false,
            );
            return;
        }

        if self.offline {
            let focused = editing == Some("offline_name");
            ui.text_field(
                x + 20.0,
                content_y,
                w - 40.0,
                58.0,
                "Player name",
                &self.new_offline,
                "3 to 16 letters, digits or _",
                focused,
            );
            targets.push((
                [x + 20.0, content_y, w - 40.0, 58.0],
                Action::Field("offline_name"),
            ));
            let count = format!("{}/16 characters", self.new_offline.chars().count());
            ui.text(
                Face::Regular,
                11.5,
                x + 20.0,
                content_y + 66.0,
                login_muted(),
                &count,
            );
            ui.hint_line(
                x + 20.0,
                content_y + 88.0,
                w - 40.0,
                "This name is local; it is not an online identity.",
                login_muted(),
            );
            if !self.message.is_empty() && !focused {
                paragraph(
                    ui,
                    x + 20.0,
                    content_y + 112.0,
                    w - 40.0,
                    &self.message,
                    theme.danger,
                    2,
                );
            }
            if focused {
                button(
                    ui,
                    targets,
                    [x + 20.0, footer_y, w - 40.0, 42.0],
                    "Done editing",
                    Action::FinishNameEdit,
                    true,
                );
                return;
            }
            button(
                ui,
                targets,
                [x + 20.0, footer_y, w - 40.0, 42.0],
                "Add offline account",
                Action::SelectOffline,
                true,
            );
            button(
                ui,
                targets,
                [x + 20.0, footer_y - 48.0, w - 40.0, 36.0],
                "Back to accounts",
                Action::Account,
                false,
            );
            return;
        }

        // Microsoft: add an account with the browser or a device code
        let card_h = 84.0;
        let card = [x + 20.0, content_y, w - 40.0, card_h];
        ui.rounded(card[0], card[1], card[2], card[3], 16.0, login_surface());
        ui.border(card[0], card[1], card[2], card[3], 16.0, login_border());
        ui.rounded(card[0] + 14.0, card[1] + 14.0, 56.0, 56.0, 16.0, login_accent_soft());
        ui.centered_fit(
            Face::SemiBold,
            24.0,
            [card[0] + 14.0, card[1] + 14.0, 56.0, 56.0],
            login_accent(),
            "M",
        );
        let name_x = card[0] + 84.0;
        ui.text(
            Face::SemiBold,
            17.0,
            name_x,
            card[1] + 16.0,
            login_text(),
            "Add a Microsoft account",
        );
        let fitted = ui.fit_text(
            Face::Regular,
            12.5,
            "Your other accounts stay saved.",
            card[2] - 100.0,
        );
        ui.text(
            Face::Regular,
            12.5,
            name_x,
            card[1] + 44.0,
            login_muted(),
            &fitted,
        );

        let mut row_y = content_y + card_h + 12.0;
        if let Some(SignInState::Code(code)) = &self.state {
            let code_h = 84.0;
            ui.rounded(
                x + 20.0,
                row_y,
                w - 40.0,
                code_h,
                14.0,
                login_surface_high(),
            );
            ui.border(x + 20.0, row_y, w - 40.0, code_h, 14.0, login_border());
            ui.text(
                Face::Regular,
                11.0,
                x + 34.0,
                row_y + 12.0,
                login_muted(),
                "SIGN-IN CODE",
            );
            ui.text(
                Face::SemiBold,
                28.0,
                x + 34.0,
                row_y + 30.0,
                login_text(),
                &code.user_code,
            );
            row_y += code_h + 10.0;
            let half = (w - 52.0) * 0.5;
            button(
                ui,
                targets,
                [x + 20.0, row_y, half, 38.0],
                "Copy code",
                Action::CopyLoginCode,
                false,
            );
            button(
                ui,
                targets,
                [x + 32.0 + half, row_y, half, 38.0],
                "Open browser",
                Action::LoginLink,
                false,
            );
            row_y += 46.0;
        } else {
            let text = "You sign in inside Microsoft's browser; this app never sees your password.";
            row_y = paragraph(ui, x + 20.0, row_y, w - 40.0, text, login_muted(), 2) + 8.0;
        }
        let message = if matches!(&self.state, Some(SignInState::Starting)) {
            "Requesting a sign-in code..."
        } else {
            self.message.as_str()
        };
        if !message.is_empty() {
            row_y = paragraph(ui, x + 20.0, row_y, w - 40.0, message, login_text(), 2) + 4.0;
        }
        if self.message.contains("not approved by Mojang") && row_y + 30.0 < footer_y - 54.0 {
            button(
                ui,
                targets,
                [x + 20.0, row_y, w - 40.0, 30.0],
                "Request Minecraft API approval",
                Action::ReviewApp,
                false,
            );
        }
        let (label, action) = if matches!(&self.state, Some(SignInState::Code(_))) {
            ("Copy code and open browser", Action::LoginLink)
        } else if self.pending() {
            ("Restart browser sign-in", Action::SignIn)
        } else {
            ("Sign in with browser", Action::SignIn)
        };
        button(
            ui,
            targets,
            [x + 20.0, footer_y, w - 40.0, 42.0],
            label,
            action,
            true,
        );
        let half = (w - 52.0) * 0.5;
        button(
            ui,
            targets,
            [x + 20.0, footer_y - 48.0, half, 38.0],
            "Device code",
            Action::DeviceSignIn,
            false,
        );
        let (second_label, second_action) = if self.pending() {
            ("Cancel", Action::CancelLogin)
        } else {
            ("Back", Action::Account)
        };
        button(
            ui,
            targets,
            [x + 32.0 + half, footer_y - 48.0, half, 38.0],
            second_label,
            second_action,
            false,
        );
    }
}

impl Drop for Login {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.cancel();
        }
    }
}

fn paragraph(ui: &mut Ui, x: f32, y: f32, w: f32, text: &str, tone: u32, max_lines: usize) -> f32 {
    let limit = (w / 6.8).max(16.0) as usize;
    let mut line = String::new();
    let mut row = 0usize;
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + word.len() + 1 > limit {
            ui.text(Face::Regular, 12.5, x, y + row as f32 * 18.0, tone, &line);
            row += 1;
            line.clear();
            if row >= max_lines {
                break;
            }
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if row < max_lines && !line.is_empty() {
        ui.text(Face::Regular, 12.5, x, y + row as f32 * 18.0, tone, &line);
        row += 1;
    }
    y + row as f32 * 18.0
}
