use serde::Serialize;

/// Version of the slot interface contract. Bump when the contract changes;
/// components/themes declare which version they target so the core can warn
/// on mismatch instead of silently breaking.
pub const SLOT_INTERFACE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct SlotDef {
    pub name: &'static str,
    pub iface_version: u32,
    pub description: &'static str,
}

/// The named slots declared by core. Everything else in the browser mounts
/// into these - nothing is hardcoded UI.
pub const SLOTS: &[SlotDef] = &[
    SlotDef { name: "top-bar", iface_version: SLOT_INTERFACE_VERSION, description: "Primary horizontal chrome area (tab strip, URL bar live here by default)" },
    SlotDef { name: "tab-strip", iface_version: SLOT_INTERFACE_VERSION, description: "Tab list region (defaults inside top-bar; sidebar layout remounts it)" },
    SlotDef { name: "bookmarks-bar", iface_version: SLOT_INTERFACE_VERSION, description: "Horizontal strip under the top bar for bookmark shortcuts" },
    SlotDef { name: "sidebar-left", iface_version: SLOT_INTERFACE_VERSION, description: "Left vertical panel" },
    SlotDef { name: "sidebar-right", iface_version: SLOT_INTERFACE_VERSION, description: "Right vertical panel" },
    SlotDef { name: "status-bar", iface_version: SLOT_INTERFACE_VERSION, description: "Bottom status strip" },
    SlotDef { name: "overlay", iface_version: SLOT_INTERFACE_VERSION, description: "Floating overlay layer (command palette, toasts)" },
];

#[derive(Debug, Clone, Serialize)]
pub struct ComponentRegistration {
    pub component: String,
    pub slot: String,
    pub iface_version: u32,
    /// Where it came from: "core" or a plugin name.
    pub source: String,
}

#[derive(Default)]
pub struct SlotRegistry {
    pub mounted: Vec<ComponentRegistration>,
}

impl SlotRegistry {
    pub fn with_defaults() -> Self {
        let mut r = SlotRegistry::default();
        for (component, slot) in [
            ("tab-strip", "top-bar"),
            ("url-bar", "top-bar"),
            ("nav-buttons", "top-bar"),
            ("bookmarks-bar", "bookmarks-bar"),
            ("status-bar", "status-bar"),
            ("command-palette", "overlay"),
        ] {
            r.mount(component.into(), slot.into(), SLOT_INTERFACE_VERSION, "core".into());
        }
        r
    }

    /// Register a component into a slot. Returns warnings (e.g. version
    /// mismatch) instead of failing silently.
    pub fn mount(
        &mut self,
        component: String,
        slot: String,
        iface_version: u32,
        source: String,
    ) -> Vec<String> {
        let mut warnings = Vec::new();
        if !SLOTS.iter().any(|s| s.name == slot) {
            // custom slots are allowed - plugins can create new regions - but note it
            warnings.push(format!("slot '{slot}' is not a core slot (custom slot created)"));
        }
        if iface_version != SLOT_INTERFACE_VERSION {
            warnings.push(format!(
                "component '{component}' targets slot interface v{iface_version}, core provides v{SLOT_INTERFACE_VERSION} - compatibility not guaranteed"
            ));
        }
        self.mounted.retain(|m| !(m.component == component && m.source == source));
        self.mounted.push(ComponentRegistration { component, slot, iface_version, source });
        warnings
    }

    pub fn unmount(&mut self, component: &str, source: &str) {
        self.mounted.retain(|m| !(m.component == component && m.source == source));
    }

    /// Removing a feature = don't mount that component. It is never instantiated.
    pub fn unmount_all_from(&mut self, source: &str) {
        self.mounted.retain(|m| m.source != source);
    }
}
