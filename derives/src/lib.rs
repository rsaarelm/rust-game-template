extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(SerializeFlags)]
pub fn derive_serialize_flags(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let gen = quote! {
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use {bitflags::Bits, util::StrExt};

                // Use '-' to signify zero flags set.
                let flags = self.bits();
                if flags == <#name as bitflags::Flags>::Bits::EMPTY {
                    return "-".serialize(serializer);
                }

                self.iter_names()
                    .map(|(s, _)| s.to_kebab_case())
                    .collect::<Vec<_>>()
                    .serialize(serializer)
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(DeserializeFlags)]
pub fn derive_deserialize_flags(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let gen = quote! {
        impl<'de> serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                use {bitflags::Bits, util::{HashMap, StrExt}};
                type B = <#name as bitflags::Flags>::Bits;

                let s = <Vec<String>>::deserialize(deserializer)?;

                if s.len() == 1 && s[0].as_str() == "-" {
                    return Ok(#name::from_bits_truncate(B::EMPTY));
                }

                let lookup = HashMap::from_iter(
                    <#name as bitflags::Flags>::FLAGS
                        .iter()
                        .map(|f| (f.name().to_kebab_case(), f.value().bits())),
                );

                let mut bits = B::EMPTY;

                for name in s {
                    let bit: B = *lookup.get(name.as_str()).ok_or_else(|| {
                        serde::de::Error::custom(format!("unknown flag {name}"))
                    })?;

                    bits |= bit;
                }

                Ok(#name::from_bits_truncate(bits))
            }
        }
    };
    gen.into()
}
