#[macro_export]
macro_rules! pg_type {
    (
        $(#[$attrs: meta])?
        struct $name: ident {
            $($field:ident : $typ: tt),*
            $(,)?
        }
    ) => {
        ::paste::paste! {
            use pgx::PostgresType;

            $(#[$attrs])?
            #[derive(PostgresType, Copy, Clone)]
            #[inoutfuncs]
            pub struct $name<'input>([<$name Data>]<'input>, Option<&'input [u8]>);

            flat_serialize_macro::flat_serialize! {
                $(#[$attrs])?
                struct [<$name Data>] {
                    header: u32,
                    $($field: $typ),*
                }
            }

            impl<'input> [<$name Data>]<'input> {
                pub unsafe fn flatten(&self) -> $name<'static> {
                    let bytes = self.to_pg_bytes();
                    let wrapped = [<$name Data>]::try_ref(bytes).unwrap().0;
                    (wrapped, bytes).into()
                }

                pub fn to_pg_bytes(&self) -> &'static [u8] {
                    let mut output = vec![];
                    self.fill_vec(&mut output);
                    unsafe {
                        set_varsize(output.as_mut_ptr() as *mut _, output.len() as i32);
                    }
                    &*output.leak()
                }
            }

            impl<'input> pgx::FromDatum for $name<'input> {
                unsafe fn from_datum(datum: Datum, is_null: bool, _: pg_sys::Oid) -> Option<Self>
                where
                    Self: Sized,
                {
                    if is_null {
                        return None;
                    }

                    let ptr = pg_sys::pg_detoast_datum_packed(datum as *mut pg_sys::varlena);
                    let data_len = varsize_any(ptr);
                    let bytes = slice::from_raw_parts(ptr as *mut u8, data_len);

                    let (data, _) = match [<$name Data>]::try_ref(bytes) {
                        Ok(wrapped) => wrapped,
                        Err(e) => error!(concat!("invalid ", stringify!($name), " {:?}, got len {}"), e, bytes.len()),
                    };

                    $name(data, Some(bytes)).into()
                }
            }

            impl<'input> pgx::IntoDatum for $name<'input> {
                fn into_datum(self) -> Option<Datum> {
                    let datum = match self.1 {
                        Some(bytes) => bytes.as_ptr() as Datum,
                        None => self.0.to_pg_bytes().as_ptr() as Datum,
                    };
                    Some(datum)
                }

                fn type_oid() -> pg_sys::Oid {
                    rust_regtypein::<Self>()
                }
            }

            impl<'input> ::std::ops::Deref for $name<'input> {
                type Target=[<$name Data>]<'input>;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl<'input> From<[<$name Data>]<'input>> for $name<'input> {
                fn from(inner: [<$name Data>]<'input>) -> Self {
                    Self(inner, None)
                }
            }

            impl<'input> From<[<$name Data>]<'input>> for Option<$name<'input>> {
                fn from(inner: [<$name Data>]<'input>) -> Self {
                    Some($name(inner, None))
                }
            }

            impl<'input> From<([<$name Data>]<'input>, &'input [u8])> for $name<'input> {
                fn from((inner, bytes): ([<$name Data>]<'input>, &'input [u8])) -> Self {
                    Self(inner, Some(bytes))
                }
            }
        }
    }
}

#[macro_export]
macro_rules! debug_inout_funcs {
    ($name:ident) => {
        impl<'input> InOutFuncs for $name<'input> {
            fn output(&self, buffer: &mut StringInfo) {
                use std::io::Write;
                let _ = write!(buffer, "{:?}", &self.0);
            }

            fn input(_input: &std::ffi::CStr) -> Self
            where
                Self: Sized,
            {
                unimplemented!(concat!("no valid TEXT input for ", stringify!($name)))
            }
        }
    };
}

#[macro_export]
macro_rules! flatten {
    ($typ:ident { $($field:ident: $value:expr),* $(,)? }) => {
        {
            let data = ::paste::paste! {
                [<$typ Data>] {
                    header: &0,
                    $(
                        $field: $value
                    ),*
                }
            };
            data.flatten()
        }
    }
}
