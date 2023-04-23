macro_rules! _def_sections {
    {
        $(
            $type_name:ident {
                $($field:ident: $type:ty = $default:expr,)*
            }
        )*
    } => {
        $(
            pub struct $type_name {
                $(pub $field: $type,)*
            }

            impl $type_name {
                const FIELD_NAMES: &'static [&'static str] = &[
                    $(std::stringify!($field),)*
                ];

                fn find_similar(&self, to: &str) -> Option<&'static str> {
                    crate::cfg::parse::most_similar(to, Self::FIELD_NAMES.iter().cloned())
                }
            }

            impl std::default::Default for $type_name {
                fn default() -> Self {
                    use crate::cfg::{parse::Value, scanner::Scanner};
                    Self {
                        $(
                            $field: <$type>::parse(&mut Scanner::new(std::concat!($default, "\n")))
                                .unwrap_or_else(|e| panic!(
                                    "invalid default value for {}::{}: {:?}",
                                    std::stringify!($type_name),
                                    std::stringify!($field),
                                    e
                                )),
                        )*
                    }
                }
            }

            impl crate::cfg::parse::Section for $type_name {
                fn set(
                    &mut self,
                    section_name: &str,
                    field: &str,
                    scanner: &mut crate::cfg::scanner::Scanner,
                ) -> crate::cfg::parse::SetResult {
                    use crate::cfg::{error::Error, parse::{Value, SetError}};
                    match field {
                        $(
                            std::stringify!($field) => self.$field = <$type>::parse(scanner)
                                .map_err(|e| SetError::InvalidValue(e))?,
                        )*
                        _ => {
                            let mut err = Error::new(format!(
                                "no field `{}` in section `{}`",
                                field,
                                section_name,
                            ));
                            if let Some(similar) = self.find_similar(&*field) {
                                // label span is set by parser
                                err = err.with_label(0..0, format!(
                                    "help: a field with a similar name exists: `{similar}`"
                                ));
                            }
                            return Err(SetError::InvalidKey(err));
                        }
                    }
                    Ok(())
                }
            }
        )*
    }
}

macro_rules! _def_parsed_config {
    {$($ident:ident: $type:ty => $section_path:expr,)*} => {
        pub struct ParsedConfig {
            $(pub $ident: $type,)*
        }

        impl ParsedConfig {
                const SECTION_NAMES: &'static [&'static str] = &[$($section_path,)*];

                fn find_similar(&self, to: &str) -> Option<&'static str> {
                    crate::cfg::parse::most_similar(to, Self::SECTION_NAMES.iter().cloned())
                }
        }

        impl std::default::Default for ParsedConfig {
            fn default() -> Self {
                Self {
                    $($ident: <$type>::default(),)*
                }
            }
        }

        impl crate::cfg::parse::Document for ParsedConfig {
            fn section(
                &mut self,
                path: &str,
            ) -> Result<&mut dyn crate::cfg::parse::Section, crate::cfg::error::Error> {
                use crate::cfg::error::Error;
                match path {
                    $($section_path => Ok(&mut self.$ident),)*
                    _ => {
                        // error line is set by parser
                        let mut err = Error::new(&format!("No such section: {path}"));
                        if let Some(similar) = self.find_similar(&*path) {
                            // label span is set by parser
                            err = err.with_label(0..0, format!(
                                "help: a section with a similar name exists: `{similar}`"
                            ));
                        }
                        Err(err)
                    }
                }
            }
        }
    }
}

/// Defines a `PrasedConfig` struct that implements the `Document` trait as well
/// as auxiliary section types implementing the `Section` trait.
///
/// Syntax:
/// ```
/// parsed_config! {
///     // Defines section types that only contain the given fields.
///     sections => {
///         // `SectionName` will be name of the resulting struct.
///         SectionName => {
///             // `Type` has to implement the `Value` trait, the default value
///             // is also parsed using it.
///             field: Type = "default_value",
///         }
///     }
///     // Defines the sections that will be in the `ParsedConfig` type.
///     // `SectionType` must implement `Section` but doesn't need to come
///     // from the `sections` block.
///     // `section_name` is the name that will be using in the config file
///     // (as `[section_name]`), this may include any alphanumeric characters,
///     // `.`, `_`, and `-`.
///     config => {
///         field_name: SectionType => "section_name",
///     }
/// }
/// ```
macro_rules! parsed_config {
    {
        sections => {
            $($section_def:tt)*
        }
        config => {
            $($ident:ident: $type:ty => $section_path:expr,)*
        }
    } => {
        crate::cfg::gen::_def_sections!($($section_def)*);
        crate::cfg::gen::_def_parsed_config!($($ident: $type => $section_path,)*);
    }
}

pub(crate) use _def_parsed_config;
pub(crate) use _def_sections;
pub(crate) use parsed_config;
