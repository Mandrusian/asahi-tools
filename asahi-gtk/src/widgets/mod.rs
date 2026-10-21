pub mod config;
pub mod frame;
pub mod picker;
pub mod settings;

pub mod battery;
pub mod battery_history;
pub mod brightness;
pub mod charge_limit;
pub mod cpu_temp;
pub mod cpu_usage;
pub mod disk_usage;
pub mod fan;
pub mod kbd_backlight;
pub mod memory;
pub mod power_draw;
pub mod profile;
pub mod quick_actions;
pub mod system_info;
pub mod uptime;
pub mod wifi_info;
pub mod wifi_signal;

use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WidgetSize {
    Small,
    Medium,
    Large,
}

impl WidgetSize {
    pub fn columns(self) -> i32 {
        match self {
            Self::Small | Self::Medium => 1,
            Self::Large => 2,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "small" => Self::Small,
            "large" => Self::Large,
            _ => Self::Medium,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WidgetInstance {
    pub id: String,
    pub widget_type: String,
    pub order: u32,
    #[serde(default = "default_size")]
    pub size: WidgetSize,
    #[serde(default)]
    pub settings: serde_json::Value,
}

fn default_size() -> WidgetSize {
    WidgetSize::Medium
}

pub struct WidgetDef {
    pub widget_type: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub build: fn(&WidgetInstance) -> gtk::Widget,
    pub settings_schema: fn(&WidgetInstance) -> Vec<settings::Field>,
    pub available: fn() -> bool,
}

pub fn registry() -> &'static [WidgetDef] {
    &[
        WidgetDef {
            widget_type: "battery",
            name: "Battery Gauge",
            description: "Circular charge indicator",
            icon: "battery-good-symbolic",
            build: battery::build,
            settings_schema: |_| vec![],
            available: || true,
        },
        WidgetDef {
            widget_type: "cpu_temp",
            name: "CPU Temperature",
            description: "Live CPU temperature sparkline",
            icon: "temperature-symbolic",
            build: cpu_temp::build,
            settings_schema: |_| {
                vec![settings::Field::Number {
                    key: "window",
                    label: "Time window (sec)",
                    min: 20.0,
                    max: 300.0,
                    step: 10.0,
                }]
            },
            available: || !crate::helper::get_hwmon_sensors().is_empty(),
        },
        WidgetDef {
            widget_type: "fan",
            name: "Fan Speed",
            description: "Fan RPM gauge or sparkline",
            icon: "fan-symbolic",
            build: fan::build,
            settings_schema: |_| {
                vec![
                    settings::Field::Number {
                        key: "fan_index",
                        label: "Fan number (1-based)",
                        min: 1.0,
                        max: 4.0,
                        step: 1.0,
                    },
                    settings::Field::Choice {
                        key: "style",
                        label: "Style",
                        options: &[("gauge", "Gauge"), ("sparkline", "Sparkline")],
                    },
                ]
            },
            available: || !crate::helper::get_hwmon_fans().is_empty(),
        },
        WidgetDef {
            widget_type: "memory",
            name: "Memory Usage",
            description: "RAM usage sparkline",
            icon: "drive-harddisk-symbolic",
            build: memory::build,
            settings_schema: |_| {
                vec![settings::Field::Number {
                    key: "window",
                    label: "Time window (sec)",
                    min: 20.0,
                    max: 300.0,
                    step: 10.0,
                }]
            },
            available: || true,
        },
        WidgetDef {
            widget_type: "wifi_signal",
            name: "Wi-Fi Signal",
            description: "Signal strength sparkline",
            icon: "network-wireless-symbolic",
            build: wifi_signal::build,
            settings_schema: |_| {
                vec![settings::Field::Number {
                    key: "window",
                    label: "Time window (sec)",
                    min: 20.0,
                    max: 300.0,
                    step: 10.0,
                }]
            },
            available: || true,
        },
        WidgetDef {
            widget_type: "profile",
            name: "Active Profile",
            description: "Current power profile",
            icon: "emblem-favorite-symbolic",
            build: profile::build,
            settings_schema: |_| vec![],
            available: || true,
        },
        WidgetDef {
            widget_type: "uptime",
            name: "Uptime",
            description: "System uptime",
            icon: "alarm-symbolic",
            build: uptime::build,
            settings_schema: |_| vec![],
            available: || true,
        },
        WidgetDef {
            widget_type: "brightness",
            name: "Brightness Slider",
            description: "Display brightness control",
            icon: "video-display-symbolic",
            build: brightness::build,
            settings_schema: |_| {
                vec![settings::Field::Number {
                    key: "step",
                    label: "Step size (raw units)",
                    min: 1.0,
                    max: 50.0,
                    step: 1.0,
                }]
            },
            available: || asahi_core::priv_daemon::is_running(),
        },
        WidgetDef {
            widget_type: "quick_actions",
            name: "Quick Actions",
            description: "Frequently used action buttons",
            icon: "system-run-symbolic",
            build: quick_actions::build,
            settings_schema: |_| {
                vec![
                    settings::Field::Toggle {
                        key: "show_charge",
                        label: "Show Override Charge",
                    },
                    settings::Field::Toggle {
                        key: "show_cancel",
                        label: "Show Cancel Override",
                    },
                    settings::Field::Toggle {
                        key: "show_kernel",
                        label: "Show Open Kernel Manager",
                    },
                    settings::Field::Toggle {
                        key: "show_calibrate",
                        label: "Show Calibrate Trackpad",
                    },
                ]
            },
            available: || true,
        },
        WidgetDef {
            widget_type: "battery_history",
            name: "Battery History",
            description: "Charge level over time",
            icon: "battery-good-symbolic",
            build: battery_history::build,
            settings_schema: |_| {
                vec![settings::Field::Number {
                    key: "window",
                    label: "Time window (sec)",
                    min: 20.0,
                    max: 300.0,
                    step: 10.0,
                }]
            },
            available: || true,
        },
        WidgetDef {
            widget_type: "cpu_usage",
            name: "CPU Usage",
            description: "Overall CPU load sparkline",
            icon: "utilities-system-monitor-symbolic",
            build: cpu_usage::build,
            settings_schema: |_| {
                vec![settings::Field::Number {
                    key: "window",
                    label: "Time window (sec)",
                    min: 20.0,
                    max: 300.0,
                    step: 10.0,
                }]
            },
            available: || true,
        },
        WidgetDef {
            widget_type: "wifi_info",
            name: "Wi-Fi Info",
            description: "SSID, IP and MAC",
            icon: "network-wireless-symbolic",
            build: wifi_info::build,
            settings_schema: |_| {
                vec![settings::Field::Toggle {
                    key: "hide_sensitive",
                    label: "Hide sensitive fields (IP/MAC)",
                }]
            },
            available: || true,
        },
        WidgetDef {
            widget_type: "power_draw",
            name: "Power Draw",
            description: "Current battery power draw",
            icon: "battery-good-symbolic",
            build: power_draw::build,
            settings_schema: |_| vec![],
            available: || true,
        },
        WidgetDef {
            widget_type: "system_info",
            name: "System Info",
            description: "Kernel and bootloader versions",
            icon: "applications-system-symbolic",
            build: system_info::build,
            settings_schema: |_| vec![],
            available: || true,
        },
        WidgetDef {
            widget_type: "kbd_backlight",
            name: "Keyboard Backlight",
            description: "Keyboard backlight slider",
            icon: "input-keyboard-symbolic",
            build: kbd_backlight::build,
            settings_schema: |_| {
                vec![settings::Field::Number {
                    key: "step",
                    label: "Step size",
                    min: 1.0,
                    max: 50.0,
                    step: 1.0,
                }]
            },
            available: || asahi_core::priv_daemon::is_running(),
        },
        WidgetDef {
            widget_type: "charge_limit",
            name: "Charge Limit",
            description: "Battery stop/resume thresholds",
            icon: "battery-good-symbolic",
            build: charge_limit::build,
            settings_schema: |_| {
                vec![settings::Field::Toggle {
                    key: "immediate",
                    label: "Apply immediately (no Save button)",
                }]
            },
            available: || true,
        },
        WidgetDef {
            widget_type: "disk_usage",
            name: "Disk Usage",
            description: "Root filesystem ring gauge",
            icon: "drive-harddisk-symbolic",
            build: disk_usage::build,
            settings_schema: |_| vec![],
            available: || true,
        },
        WidgetDef {
            widget_type: "gpu",
            name: "GPU Usage",
            description: "Not available on this system (no sysfs exposed)",
            icon: "video-display-symbolic",
            build: |_| {
                crate::helper::unavailable_widget("GPU telemetry not exposed by this kernel")
            },
            settings_schema: |_| vec![],
            available: || false,
        },
        WidgetDef {
            widget_type: "network_throughput",
            name: "Network Throughput",
            description: "Not implemented yet",
            icon: "network-wireless-symbolic",
            build: |_| crate::helper::unavailable_widget("Throughput sampling not implemented"),
            settings_schema: |_| vec![],
            available: || false,
        },
        WidgetDef {
            widget_type: "per_core_cpu",
            name: "Per-Core CPU",
            description: "Not implemented yet",
            icon: "utilities-system-monitor-symbolic",
            build: |_| crate::helper::unavailable_widget("Per-core sampling not implemented"),
            settings_schema: |_| vec![],
            available: || false,
        },
    ]
}

pub fn find_def(widget_type: &str) -> Option<&'static WidgetDef> {
    registry().iter().find(|d| d.widget_type == widget_type)
}

pub fn default_layout() -> Vec<WidgetInstance> {
    let defaults = [
        "battery",
        "cpu_temp",
        "fan",
        "memory",
        "wifi_signal",
        "profile",
        "uptime",
        "brightness",
        "quick_actions",
    ];
    defaults
        .iter()
        .enumerate()
        .map(|(i, t)| WidgetInstance {
            id: format!("{}_{}", t, i),
            widget_type: t.to_string(),
            order: i as u32,
            size: WidgetSize::Medium,
            settings: serde_json::json!({}),
        })
        .collect()
}

pub fn new_instance(widget_type: &str, order: u32) -> WidgetInstance {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    WidgetInstance {
        id: format!("{}_{}", widget_type, nanos),
        widget_type: widget_type.to_string(),
        order,
        size: WidgetSize::Medium,
        settings: serde_json::json!({}),
    }
}

pub type ApplySettings = Rc<dyn Fn(String, serde_json::Value)>;
pub type OnRemove = Rc<dyn Fn(String)>;
pub type OnMove = Rc<dyn Fn(String, i32)>;
pub type OnAdd = Rc<dyn Fn(&'static str)>;
