use gloo::{console, timers::callback::Interval};
use std::borrow::Borrow;
use std::convert::Into;
use std::time::Duration;
use wasm_bindgen::{JsValue, JsCast};
use web_sys::{Notification, NotificationOptions, InputEvent, HtmlInputElement, HtmlAudioElement, window};
use yew::{html, Component, Context, Html};
use serde::{Serialize, Deserialize};

static TICK_INTERVAL: u32 = 1000;

#[derive(Serialize, Deserialize)]
struct Period {
    name: String,
    duration: Duration,
}

#[derive(Clone)]
pub enum Msg {
    Start,
    Reset,
    Pause,
    Finish,
    Resume,
    Tick(u32),
    UpdateName(usize, String),
    UpdateMinutes(usize, u64),
    UpdateSeconds(usize, u64),
}

impl Msg {
    fn to_str(&self) -> &'static str {
        match *self {
            Msg::Start => "Start",
            Msg::Reset => "Reset",
            Msg::Pause => "Pause",
            Msg::Finish => "Finish",
            Msg::Resume => "Resume",
            _ => panic!(),
        }
    }
}

pub struct App {
    messages: Vec<&'static str>,
    interval: Option<(Interval, u64, f64)>,
    progress: Option<Duration>,
    periods: Vec<Period>,
    current_period: usize,
}

impl App {
    fn get_current_period_length(&self) -> Duration {
        self.periods
            .get(self.current_period)
            .map(|period| period.duration)
            .unwrap_or(Duration::from_secs(25 * 60))
    }

    fn reset(&mut self) {
        self.interval = None;
        self.progress = None;
    }

    fn notify(&mut self, milliseconds_update: u32) {
        let current_period_length = self.get_current_period_length();
        if let Some(prog) = self.progress.as_mut() {
            let minutes_left_before: f64 = (current_period_length.as_secs_f64() - prog.as_secs_f64()) / 60.;
            let (_, tick_count, tick_start) = self.interval.as_mut().expect("in notify, interval must not be none");
            update_progress(prog, milliseconds_update, tick_count, *tick_start);
            let minutes_left_now: f64 = (current_period_length.as_secs_f64() - prog.as_secs_f64()) / 60.;
            console::log!("minutes_left_now", minutes_left_now);
            if minutes_left_now <= 0. {
                let notification_period_len = 5.;
                let notification_periods_since_over = -minutes_left_now / notification_period_len;
                console::log!("notification_periods_since_over", notification_periods_since_over);
                let notification_periods_before = -minutes_left_before / notification_period_len;
                console::log!("notification_periods_before", notification_periods_before);
                if notification_periods_since_over.floor() > notification_periods_before.floor() {
                    // We should notify
                    let period_name =
                        if let Some(period) = self.periods.get(self.current_period) {
                            period.name.clone()
                        } else {
                            "Work".to_string()
                        };
                        let mut note_options = NotificationOptions::new();
                        let note_message = if notification_periods_since_over >= 1. {
                            format!("{} has been over for {} minutes", period_name,
                                (notification_periods_since_over * notification_period_len) as i64)
                        } else {
                            format!("{} is over", period_name)
                        };
                        note_options.body(&note_message);
                        log_error(
                            Notification::new_with_options("Done!", &note_options),
                            "Could not show notification",
                        );
                        if let Ok(audio) = HtmlAudioElement::new_with_src("notification.mp3") {
                            audio.set_autoplay(true);
                        }
                }
            }
        }
    }

    fn save_periods(&self) -> Result<(), JsValue> {
        let window = window().ok_or("no window")?;
        let storage = window.local_storage()?.ok_or("no local storage")?;
        let content = serde_json::to_string(&self.periods).map_err(|_| "Error serializing periods")?;
        storage.set("pomyu_periods", &content)?;
        Ok(())
    }

    fn load_periods(&mut self) -> Result<(), JsValue> {
        let window = window().ok_or("no window")?;
        let storage = window.local_storage()?.ok_or("no local storage")?;
        let content = storage.get("pomyu_periods")?.ok_or("pomyu_periods not found")?;
        self.periods = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn log_error<V, E: Into<JsValue>, S: Borrow<str>>(result: Result<V, E>, err_msg: S) -> Option<V> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            console::error!(err_msg.borrow());
            let e_js: JsValue = e.into();
            console::error!(e_js);
            None
        }
    }
}

fn format_duration(duration: Duration) -> String {
    format!(
        "{:02}:{:02}",
        duration.as_secs() / 60,
        duration.as_secs() % 60
    )
}

fn get_utc_millis() -> f64 {
    js_sys::Date::new_0().get_time()
}

fn update_progress(progress: &mut Duration, expected_millisecond_update: u32, tick_count: &mut u64, tick_start: f64) {
    let update = Duration::from_millis(expected_millisecond_update as u64);
    let time_now = get_utc_millis();
    let new_progress = Duration::from_millis((time_now - tick_start) as u64);
    // If the clock decides to go backward, *progress + update will win.
    // If the app is lagging and ticks are happening less frequently than requested, new_progress
    // will win.
    *progress = Duration::max(new_progress, *progress + update);
    *tick_count += 1;
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        let mut this = Self {
            messages: Vec::new(),
            interval: None,
            progress: None,
            periods: vec![
                Period {
                    name: "Focus".to_string(),
                    duration: Duration::from_secs(25 * 60),
                },
                Period {
                    name: "Small break".to_string(),
                    duration: Duration::from_secs(5 * 60),
                },
                Period {
                    name: "Focus".to_string(),
                    duration: Duration::from_secs(25 * 60),
                },
                Period {
                    name: "Full break".to_string(),
                    duration: Duration::from_secs(15 * 60),
                },
            ],
            current_period: 0,
        };
        log_error(this.load_periods(), "Could not load periods");
        this
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Start => {
                self.update(ctx, Msg::Resume);
                self.progress = Some(Duration::ZERO.clone());

                self.messages.clear();
                console::clear!();

                self.messages.push("Interval started!");
                true
            }
            Msg::Resume => {
                log_error(
                    Notification::request_permission(),
                    "Could not get notification permissions",
                );

                let handle = {
                    let link = ctx.link().clone();
                    Interval::new(TICK_INTERVAL, move || {
                        link.send_message(Msg::Tick(TICK_INTERVAL))
                    })
                };
                self.interval = Some((handle, 0, get_utc_millis()));

                self.messages.push("resume");
                true
            }
            Msg::Reset => {
                self.reset();
                self.messages.push("reset");
                true
            }
            Msg::Pause => {
                self.interval = None;
                self.messages.push("pause");
                true
            }
            Msg::Finish => {
                self.reset();
                self.messages.push("Finish!");
                self.interval = None;
                self.progress = None;
                self.current_period += 1;
                self.current_period %= usize::max(1, self.periods.len());
                true
            }
            Msg::Tick(milliseconds) => {
                self.notify(milliseconds);
                true
            }

            Msg::UpdateName(period_number, new_name) => {
                if let Some(period) = self.periods.get_mut(period_number) {
                    period.name = new_name;
                }
                log_error(self.save_periods(), "Could not save periods");
                true
            }

            Msg::UpdateMinutes(period_number, minutes) => {
                if let Some(period) = self.periods.get_mut(period_number) {
                    // Subtract existing minutes
                    period.duration -= Duration::from_secs(60 * (period.duration.as_secs() / 60));
                    // Add new minutes
                    period.duration += Duration::from_secs(minutes as u64 * 60);
                }
                log_error(self.save_periods(), "Could not save periods");
                true
            }

            Msg::UpdateSeconds(period_number, seconds) => {
                if let Some(period) = self.periods.get_mut(period_number) {
                    // Round down to nearest minute
                    period.duration = Duration::from_secs(60 * (period.duration.as_secs() / 60));
                    period.duration += Duration::from_secs(seconds as u64);
                }
                log_error(self.save_periods(), "Could not save periods");
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let center_button_msg;
        if self.progress.is_some() {
            if self.interval.is_some() {
                match self.progress {
                    Some(prog) if prog >= self.get_current_period_length() => {
                        center_button_msg = Msg::Finish;
                    }
                    _ => {
                        center_button_msg = Msg::Pause;
                    }
                }
            } else {
                center_button_msg = Msg::Resume;
            }
        } else {
            center_button_msg = Msg::Start;
        }
        let center_button_contents = center_button_msg.to_str();
        html! {
            <div class="container-fluid main-container">
                <div class="time">
                    {
                        if let Some(prog) = self.progress {
                            format_duration(prog)
                        } else {
                            "00:00".to_string()
                        }
                    }
                </div>
                <progress
                    value={
                        self.progress.map(|p| p.as_millis()).unwrap_or(0).to_string()
                    }
                    max={ self.get_current_period_length().as_millis().to_string() }>
                </progress>
                <div class="grid">
                    <button disabled={self.progress.is_none()}
                            onclick={ctx.link().callback(|_| Msg::Reset)}>
                        { "Reset" }
                    </button>
                    <button onclick={ctx.link().callback( move |_| center_button_msg.clone()) }>
                        {
                            center_button_contents
                        }
                    </button>
                    <button onclick={ctx.link().callback(|_| Msg::Finish)}>
                        { "Skip" }
                    </button>
                </div>
                <div class="grid periods">
                    <div>
                    { for self.periods.iter().enumerate().map(|(i, period)| {
                        html!{
                            <div class={
                                {
                                    if i == self.current_period {
                                        "grid current-period".to_string()
                                    } else {
                                        "grid".to_string()
                                    }
                                }
                            }>
                                <div>
                                    <input type="text"
                                        oninput={ctx.link().batch_callback(move |e: InputEvent| {
                                            e.target()
                                             .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                                             .map(|el| Msg::UpdateName(i, el.value()))
                                        })}
                                        value={ period.name.clone() }/>
                                </div>
                                <div class="grid">
                                    <input type="number" min=0
                                    oninput={ctx.link().batch_callback(move |e: InputEvent| {
                                        e.target()
                                            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                                            .and_then(|el| {el.value().parse().ok()})
                                            .map(|val| Msg::UpdateMinutes(i, val))
                                    })}
                                    value={ (period.duration.as_secs() / 60).to_string() }/>
                                    <input type="number" min=0 max=60
                                    oninput={ctx.link().batch_callback(move |e: InputEvent| {
                                        e.target()
                                            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                                            .and_then(|el| {el.value().parse().ok()})
                                            .map(|val| Msg::UpdateSeconds(i, val))
                                    })}
                                    value={ (period.duration.as_secs() % 60).to_string() }/>
                                </div>
                            </div>
                        }
                      })
                    }
                    </div>
                </div>
            </div>
        }
    }
}

fn main() {
    yew::start_app::<App>();
}
