use crate::{
    Result, fail,
    state::{Store, atomic_json, read_json},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Navigation {
    pub schema_version: u32,
    pub page: i32,
    pub wizard: bool,
    pub step: i32,
}
impl Default for Navigation {
    fn default() -> Self {
        Self {
            schema_version: 1,
            page: 0,
            wizard: true,
            step: 0,
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    Navigate,
    Review,
    Execute,
    Cancel,
    Wait,
}
impl Navigation {
    pub fn load(store: &Store) -> Result<Self> {
        let path = store.root.join("ui-preferences.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut nav: Self = read_json(&path)?;
        nav.validate()?;
        nav.wizard = true;
        if matches!(nav.page, 3..=5) {
            nav.page = 6;
        }
        if nav.page == 0 {
            nav.step = 0;
        }
        nav.step = match nav.page {
            0 => 0,
            2 => 1,
            1 => 2,
            6 => 3,
            _ => nav.step,
        };
        Ok(nav)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1
            || !(0..=8).contains(&self.page)
            || !(0..=4).contains(&self.step)
        {
            return Err(fail(
                "Preferências de navegação incompatíveis; seleção e fila foram preservadas.",
            ));
        }
        Ok(())
    }
    pub fn save(&self, store: &Store) -> Result<()> {
        self.validate()?;
        atomic_json(&store.root.join("ui-preferences.json"), self)
    }
    pub fn visit(&mut self, page: i32) {
        self.page = if matches!(page, 3..=5) { 6 } else { page };
        if self.wizard {
            match self.page {
                2 => self.step = 1,
                1 => self.step = 2,
                6 => self.step = 3,
                _ => {}
            }
        }
    }
    pub fn primary(
        &mut self,
        pc_ready: bool,
        plan_ready: bool,
        busy: bool,
        all_done: bool,
    ) -> Next {
        if busy {
            return if self.page == 7 {
                Next::Cancel
            } else {
                Next::Wait
            };
        }
        if !pc_ready {
            return Next::Wait;
        }
        match self.page {
            0 => {
                self.wizard = true;
                if self.step == 0 {
                    self.step = 1;
                }
                self.page = step_page(self.step);
                Next::Navigate
            }
            6 => {
                if plan_ready {
                    Next::Execute
                } else {
                    Next::Review
                }
            }
            7 if all_done => {
                *self = Self::default();
                Next::Navigate
            }
            7 => Next::Review,
            8 => {
                self.page = 7;
                Next::Navigate
            }
            2 if self.wizard => {
                self.visit(1);
                Next::Navigate
            }
            1 if self.wizard => Next::Review,
            _ => Next::Review,
        }
    }
    pub fn back(&mut self) {
        self.page = match self.page {
            2 => 0,
            1 if self.wizard => 2,
            3..=6 if self.wizard => 1,
            8 => 7,
            _ => 0,
        };
        if self.page != 0 {
            self.visit(self.page)
        } else {
            self.step = 0;
        }
    }
}
pub fn step_page(step: i32) -> i32 {
    match step {
        1 => 2,
        2 => 1,
        3 | 4 => 6,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wizard_routes_directly_from_apps_to_review() {
        let mut n = Navigation::default();
        assert_eq!(n.primary(false, false, false, false), Next::Wait);
        assert_eq!(n.primary(true, false, false, false), Next::Navigate);
        assert_eq!((n.page, n.step), (2, 1));
        n.primary(true, false, false, false);
        assert_eq!((n.page, n.step), (1, 2));
        assert_eq!(n.primary(true, false, false, false), Next::Review);
        n.visit(6);
        assert_eq!((n.page, n.step), (6, 3));
        assert_eq!(n.primary(true, true, false, false), Next::Execute);
    }
    #[test]
    fn back_and_resume_preserve_navigation() {
        let mut n = Navigation {
            wizard: true,
            page: 6,
            step: 4,
            ..Default::default()
        };
        n.back();
        assert_eq!((n.page, n.step), (1, 2));
        n.visit(0);
        n.primary(true, false, false, false);
        assert_eq!((n.page, n.step), (1, 2));
    }
    #[test]
    fn navigation_is_separate_from_profile() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store {
            root: dir.path().into(),
        };
        let profile = crate::state::Profile::default();
        store.save_profile(&profile).unwrap();
        let n = Navigation {
            wizard: true,
            page: 4,
            step: 3,
            ..Default::default()
        };
        n.save(&store).unwrap();
        assert_eq!(
            Navigation::load(&store).unwrap(),
            Navigation {
                page: 6,
                step: 3,
                ..n
            }
        );
        assert_eq!(store.load_profile().unwrap(), profile);
    }
    #[test]
    fn loading_and_busy_cannot_start_installs() {
        let mut n = Navigation {
            page: 6,
            ..Default::default()
        };
        assert_eq!(n.primary(false, true, false, false), Next::Wait);
        assert_eq!(n.primary(true, true, true, false), Next::Wait);
        n.page = 7;
        assert_eq!(n.primary(true, true, true, false), Next::Cancel);
    }
    #[test]
    fn future_ui_settings_are_rejected() {
        assert!(
            Navigation {
                schema_version: 9,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Navigation {
                page: 99,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
