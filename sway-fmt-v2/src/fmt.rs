use crate::utils::{
    indent_style::Shape, newline_style::apply_newline_style, program_type::insert_program_type,
};
use std::{path::Path, sync::Arc};
use sway_core::BuildConfig;

pub use crate::{
    config::manifest::Config,
    error::{ConfigError, FormatterError},
};

#[derive(Debug, Default)]
pub struct Formatter {
    pub shape: Shape,
    pub config: Config,
}

pub type FormattedCode = String;

pub trait Format {
    fn format(
        &self,
        formatted_code: &mut FormattedCode,
        formatter: &mut Formatter,
    ) -> Result<(), FormatterError>;
}

impl Formatter {
    pub fn from_dir(dir: &Path) -> Result<Self, ConfigError> {
        let config = match Config::from_dir(dir) {
            Ok(config) => config,
            Err(ConfigError::NotFound) => Config::default(),
            Err(e) => return Err(e),
        };
        let shape = Shape::default();
        Ok(Self { config, shape })
    }
    pub fn format(
        &mut self,
        src: Arc<str>,
        build_config: Option<&BuildConfig>,
    ) -> Result<FormattedCode, FormatterError> {
        let path = build_config.map(|build_config| build_config.canonical_root_module());
        let src_len = src.len();
        let module = sway_parse::parse_file(src, path)?;
        // Get parsed items
        let items = module.items;
        // Get the program type (script, predicate, contract or library)
        let program_type = module.kind;

        // Formatted code will be pushed here with raw newline stlye.
        // Which means newlines are not converted into system-specific versions by apply_newline_style
        // Use the length of src as a hint of the memory size needed for raw_formatted_code,
        // which will reduce the number of reallocations
        let mut raw_formatted_code = String::with_capacity(src_len);

        // Insert program type to the formatted code.
        insert_program_type(&mut raw_formatted_code, program_type);

        // Insert parsed & formatted items into the formatted code.
        let mut iter = items.iter().peekable();
        while let Some(item) = iter.next() {
            // format Annotated<ItemKind>
            item.format(&mut raw_formatted_code, self)?;
            if iter.peek().is_some() {
                raw_formatted_code.push('\n');
            }
        }

        let mut formatted_code = String::from(&raw_formatted_code);
        apply_newline_style(
            // The user's setting for `NewlineStyle`
            self.config.whitespace.newline_style,
            &mut formatted_code,
            &raw_formatted_code,
        );
        Ok(formatted_code)
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, Formatter};
    use crate::utils::indent_style::Shape;
    use std::sync::Arc;

    fn get_formatter(config: Config, shape: Shape) -> Formatter {
        Formatter { config, shape }
    }

    #[test]
    fn test_const() {
        let sway_code_to_format = r#"contract;
pub const TEST:u16=10;"#;
        let correct_sway_code = r#"contract;

pub const TEST: u16 = 10;"#;
        let mut formatter = Formatter::default();
        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }

    #[test]
    fn test_struct_single_line_alignment() {
        let sway_code_to_format = r#"contract;
pub struct Foo {
    bar: u64,
    baz: bool,
}
"#;
        let correct_sway_code = r#"contract;

pub struct Foo { bar: u64, baz: bool }"#;
        let mut config = Config::default();
        config.structures.struct_lit_single_line = true;
        config.structures.struct_field_align_threshold = 40;
        config.whitespace.max_width = 300;
        let mut formatter = get_formatter(config, Shape::default());
        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }
    #[test]
    fn test_struct_multiline_line_alignment() {
        let sway_code_to_format = r#"contract;
pub struct Foo {
   barbazfoo: u64,
   baz  : bool,
}
"#;
        let correct_sway_code = r#"contract;

pub struct Foo {
    barbazfoo: u64,
    baz      : bool,
}"#;
        let mut config = Config::default();
        config.structures.struct_field_align_threshold = 40;
        let mut formatter = get_formatter(config, Shape::default());
        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }
    #[test]
    fn test_struct_single_line() {
        let sway_code_to_format = r#"contract;
pub struct Foo {
    bar: u64,
    baz: bool,
}
"#;
        let correct_sway_code = r#"contract;

pub struct Foo { bar: u64, baz: bool }"#;
        let mut config = Config::default();
        config.structures.struct_lit_single_line = true;
        config.whitespace.max_width = 300;
        let mut formatter = get_formatter(config, Shape::default());
        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }

    #[test]
    fn test_struct_multi_line() {
        let sway_code_to_format = r#"contract;
pub struct Foo {
    bar: u64,
    baz: bool,
}
"#;
        let correct_sway_code = r#"contract;

pub struct Foo {
    bar: u64,
    baz: bool,
}"#;
        let mut formatter = Formatter::default();
        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }

    #[test]
    fn test_enum_without_variant_alignment() {
        let sway_code_to_format = r#"contract;

enum Color {
    Blue: (), Green: (),
            Red: (),
    Silver: (),
                    Grey: (), }
        "#;
        let correct_sway_code = r#"contract;

enum Color {
    Blue : (),
    Green : (),
    Red : (),
    Silver : (),
    Grey : (),
}"#;
        let mut formatter = Formatter::default();
        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }
    #[test]
    fn test_enum_with_variant_alignment() {
        let sway_code_to_format = r#"contract;

enum Color {
    Blue: (), Green: (),
            Red: (),
    Silver: (),
                    Grey: (), }
        "#;
        let correct_sway_code = r#"contract;

enum Color {
    Blue   : (),
    Green  : (),
    Red    : (),
    Silver : (),
    Grey   : (),
}"#;

        // Creating a config with enum_variant_align_threshold that exceeds longest variant length
        let mut formatter = Formatter::default();
        formatter.config.structures.enum_variant_align_threshold = 20;

        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }
    #[test]
    fn test_item_abi() {
        let sway_code_to_format = r#"contract;

abi StorageMapExample {
    #[storage(write,)]fn insert_into_map1(key: u64, value: u64);

fn hello(key: u64, value: u64);
}"#;
        let correct_sway_code = r#"contract;

abi StorageMapExample {
    #[storage(write)]
    fn insert_into_map1(key: u64, value: u64);

    fn hello(key: u64, value: u64);
}"#;
        let mut formatter = Formatter::default();
        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }

    #[test]
    fn test_multi_items() {
        let sway_code_to_format = r#"contract;

pub const TEST: u16 = 10;
pub const TEST1: u16 = 10;"#;
        let correct_sway_code = r#"contract;

pub const TEST: u16 = 10;
pub const TEST1: u16 = 10;"#;

        let mut formatter = Formatter::default();
        let formatted_sway_code =
            Formatter::format(&mut formatter, Arc::from(sway_code_to_format), None).unwrap();
        assert!(correct_sway_code == formatted_sway_code)
    }
}
