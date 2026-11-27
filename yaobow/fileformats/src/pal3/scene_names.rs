use std::collections::{HashMap, hash_map::Entry};

#[derive(Default)]
pub struct SceneNames {
    scenes: HashMap<String, HashMap<String, String>>,
}

impl SceneNames {
    pub fn parse(text: &str) -> Self {
        let mut scenes = HashMap::new();
        let mut current = None;
        for line in text.lines() {
            let line = line.split(';').next().unwrap_or("").trim();
            if let Some((keyword, scene)) = line.split_once(',') {
                if keyword.trim().eq_ignore_ascii_case("BEGIN") {
                    current = None;
                    let scene = scene.trim().to_ascii_lowercase();
                    if !scene.is_empty() {
                        if let Entry::Vacant(entry) = scenes.entry(scene.clone()) {
                            entry.insert(HashMap::<String, String>::new());
                            current = Some(scene);
                        }
                    }
                    continue;
                }
            }
            if line.eq_ignore_ascii_case("END") {
                current = None;
                continue;
            }
            let Some(scene) = current.as_ref().and_then(|scene| scenes.get_mut(scene)) else {
                continue;
            };
            let Some((key, value)) = line.split_once('$') else {
                continue;
            };
            let Some((value, _)) = value.split_once('&') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            if !key.is_empty() && !value.is_empty() {
                scene.entry(key).or_insert_with(|| value.to_string());
            }
        }
        Self { scenes }
    }

    pub fn lookup(&self, scene: &str, block: &str) -> Option<&str> {
        let names = self.scenes.get(&scene.to_ascii_lowercase())?;
        names
            .get(&block.to_ascii_lowercase())
            .or_else(|| names.get("default"))
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_blocks_and_scene_defaults_without_leaking_between_sections() {
        let names = SceneNames::parse(
            "BEGIN,M01 ; scene section\r\n\
             default\t$\u{74a7}\u{5c71}&\r\n\
             2\t$\u{74a7}\u{5c71}\u{5c71}\u{6d1e}\u{58f9}&\r\nEND\r\n\
             orphan $not a scene&\r\n\
             begin,Q01\n1n01a $Shop&\nDEFAULT $Town&\nend\n",
        );
        assert_eq!(
            names.lookup("m01", "2"),
            Some("\u{74a7}\u{5c71}\u{5c71}\u{6d1e}\u{58f9}")
        );
        assert_eq!(names.lookup("M01", "missing"), Some("\u{74a7}\u{5c71}"));
        assert_eq!(names.lookup("q01", "1N01A"), Some("Shop"));
        assert_eq!(names.lookup("Q01", "orphan"), Some("Town"));
        assert_eq!(names.lookup("unknown", "2"), None);
    }

    #[test]
    fn keeps_first_native_matches_and_ignores_incomplete_entries() {
        let names = SceneNames::parse(
            "BEGIN,Q01\n1 $First&\n1 $Second&\n2 $unterminated\n3 $&\nEND\n\
             BEGIN,q01\n1 $Duplicate section&\ndefault $Wrong default&\nEND\n\
             BEGIN,M01\n1 $Other scene&",
        );
        assert_eq!(names.lookup("q01", "1"), Some("First"));
        assert_eq!(names.lookup("q01", "2"), None);
        assert_eq!(names.lookup("q01", "3"), None);
        assert_eq!(names.lookup("m01", "1"), Some("Other scene"));
    }
}
