use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(NiObjectType, attributes(prop))]
pub fn ni_object_type(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, .. } = parse_macro_input!(input);
    let type_ident = syn::Ident::new(&format!("NiType{}", ident), ident.span());
    let read_ident = syn::Ident::new(&format!("read_{}", ident), ident.span());
    let write_ident = syn::Ident::new(&format!("write_{}", ident), ident.span());

    quote::quote! {
        #[allow(non_upper_case_globals)]
        const #type_ident: NiType = NiType {
            name: stringify!(#ident),
            read: #read_ident,
            write: #write_ident,
        };

        #[allow(non_snake_case)]
        fn #read_ident(reader: &mut Cursor<Vec<u8>>, block_size: u32) -> BinResult<Box<dyn NiObject>> {
            Ok(Box::new(#ident::read_args(reader, block_size)?))
        }

        #[allow(non_snake_case)]
        fn #write_ident(object: &dyn NiObject, writer: &mut Cursor<Vec<u8>>) -> BinResult<()> {
            #ident::write(
                object.as_any().downcast_ref::<#ident>().unwrap(),
                writer,
            )
        }

        impl crate::nif::NiObject for #ident {
            fn ni_type(&self) -> &'static super::NiType {
                &#type_ident
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    }
    .into()
}
