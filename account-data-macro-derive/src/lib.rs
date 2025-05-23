use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(AccountData)]
pub fn account_data_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_account_data_derive(&ast)
}

fn impl_account_data_derive(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let quoted = quote! {
        impl AccountData for #name {
            fn account_data(&self) -> Vec<u8> {
                let mut data = vec![];
                data.extend_from_slice(Self::DISCRIMINATOR);
                data.extend_from_slice(self.try_to_vec().unwrap().as_ref());
                data
            }
        }
    };
    quoted.into()
}
