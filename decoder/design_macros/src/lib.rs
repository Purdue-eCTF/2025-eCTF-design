use proc_macro::TokenStream;

use proc_macro2::Span;
use quote::quote;
use rand::{rngs::OsRng, seq::SliceRandom, Rng};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::{Gt, Lt},
    BinOp, Error, Expr, ExprPath, Generics, Ident, Path, PathArguments, PathSegment, Result, Token,
    TypeBareFn,
};

/// The total amount the counter will be incremented throughout all the checks
const TOTAL_INCREMENT_AMOUNT: u8 = 100;

struct Args {
    /// Condition to evaluate
    cond: Expr,
    success_type: TypeBareFn,
    /// Function to call on success
    success_fn: ExprPath,
    /// Arguments to the success function
    success_args: Punctuated<Expr, Token![,]>,
    /// Action to perform on error
    error: Expr,
    /// The random number generator used for random operations
    rng: Expr,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self> {
        let cond = Expr::parse(input)?;
        let _ = <Token![,]>::parse(input)?;

        let success_type = TypeBareFn::parse(input)?;
        let _ = <Token![,]>::parse(input)?;

        let success_fn = ExprPath::parse(input)?;
        let _ = <Token![,]>::parse(input)?;

        let Expr::Tuple(success_args) = Expr::parse(input)? else {
            return Err(Error::new(
                Span::call_site(),
                "expected a tuple of arguments to call success function with",
            ));
        };
        let _ = <Token![,]>::parse(input)?;

        let error = Expr::parse(input)?;
        let _ = <Token![,]>::parse(input)?;

        let rng = Expr::parse(input)?;
        // this one is optional
        let _ = <Token![,]>::parse(input);

        Ok(Args {
            cond,
            success_type,
            success_fn,
            success_args: success_args.elems,
            error,
            rng,
        })
    }
}

/// the function that will panic on an incorrect jump table jump
fn panic_fn() -> Expr {
    let glitch_fail = PathSegment {
        ident: Ident::new("glitch_fail", Span::call_site()),
        arguments: PathArguments::None,
    };

    let mut segments = Punctuated::new();
    segments.push(glitch_fail);

    let path = Path {
        leading_colon: None,
        segments,
    };

    Expr::Path(ExprPath {
        attrs: Vec::new(),
        qself: None,
        path,
    })
}

/// Creates a mutate function that must be called n times on 0 to reach a specified constant
///
/// Used for glitch protection
#[proc_macro]
pub fn create_mutations(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::LitInt);
    let number: usize = input.base10_parse().unwrap();
    let op_idx: u8 = OsRng.gen();
    let mutation_val: i32 = OsRng.gen::<u8>() as i32;

    let op: BinOp = match op_idx % 2 {
        0 => {
            let minus = Token![-](Span::call_site());
            BinOp::Sub(minus)
        }
        1 => {
            let add = Token![+](Span::call_site());
            BinOp::Add(add)
        }
        _ => todo!(),
    };

    let mut final_val = quote!(0);

    let mutate: proc_macro2::TokenStream = quote!(
        #op #mutation_val
    );

    for _ in 0..number {
        final_val.extend(mutate.clone());
    }

    quote!(
        const VERIFIED_VALUE: i32 = #final_val;
        fn mutate(val: &mut i32) {
            *val = *val #mutate;
        }
    )
    .into()
}

#[proc_macro]
pub fn check_or_error_jump_table(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Args);

    let jump_table_correct_index: u8 = OsRng.gen();

    let (jump_table_start_index, op) = if jump_table_correct_index < 128 {
        let minus_eq = Token![-=](Span::call_site());
        (
            jump_table_correct_index + TOTAL_INCREMENT_AMOUNT,
            BinOp::SubAssign(minus_eq),
        )
    } else {
        let add_eq = Token![+=](Span::call_site());
        (
            jump_table_correct_index - TOTAL_INCREMENT_AMOUNT,
            BinOp::AddAssign(add_eq),
        )
    };

    let mut jump_table_punctuated: Punctuated<Expr, Token![,]> = Punctuated::new();
    for i in 0..256 {
        if i == jump_table_correct_index.into() {
            jump_table_punctuated.push(Expr::Path(input.success_fn.clone()));
        } else {
            jump_table_punctuated.push(panic_fn());
        }
    }

    let success_fn_type = input.success_type;

    let mut success_inputs = success_fn_type.inputs.clone();
    for arg in success_inputs.iter_mut() {
        arg.name = Some((
            Ident::new("_", Span::call_site()),
            Token![:](Span::call_site()),
        ));
    }

    let success_output = success_fn_type.output.clone();

    let lifetimes = if let Some(lifetimes) = success_fn_type.lifetimes.as_ref() {
        lifetimes.lifetimes.clone()
    } else {
        Punctuated::new()
    };

    let generics = Generics {
        lt_token: Some(Lt {
            spans: [Span::call_site()],
        }),
        params: lifetimes,
        gt_token: Some(Gt {
            spans: [Span::call_site()],
        }),
        where_clause: None,
    };

    let success_args = input.success_args;

    let cond = input.cond;
    let error = input.error;
    let rng = input.rng;

    quote! {{
        fn glitch_fail #generics(#success_inputs) #success_output {
            panic!("glitching detected");
        }

        static JUMP_TABLE: [#success_fn_type; 256] = [#jump_table_punctuated];
        let mut glitch_token: u8 = core::hint::black_box(#jump_table_start_index);

        core::hint::black_box(design_utils::anti_hardware::rand_ops!(#rng));

        if !core::hint::black_box(#cond) {
            #error
        } else {
            core::hint::black_box({glitch_token #op 2});

            core::hint::black_box(design_utils::anti_hardware::rand_ops!(#rng));

            if core::hint::black_box(#cond) {
                core::hint::black_box({glitch_token #op 3});
                if core::hint::black_box(#cond) {
                    core::hint::black_box({glitch_token #op 5});
                    core::hint::black_box(design_utils::anti_hardware::rand_ops!(#rng));
                    core::hint::black_box({glitch_token #op 7});
                    if core::hint::black_box(#cond) {
                        core::hint::black_box({glitch_token #op 11});
                        if core::hint::black_box(#cond) {
                            core::hint::black_box({glitch_token #op 13});
                            core::hint::black_box(design_utils::anti_hardware::rand_ops!(#rng));
                            core::hint::black_box({glitch_token #op 17});
                            if core::hint::black_box(#cond) {
                                core::hint::black_box({glitch_token #op 19});
                                if core::hint::black_box(#cond) {
                                    core::hint::black_box({glitch_token #op 23});
                                    let function = core::hint::black_box(JUMP_TABLE[glitch_token as usize]);
                                    function(#success_args)
                                } else {
                                    panic!("glitching detected");
                                }
                            } else {
                                panic!("glitching detected");
                            }
                        } else {
                            panic!("glitching detected");
                        }
                    } else {
                        panic!("glitching detected");
                    }
                } else {
                    panic!("glitching detected");
                }
            } else {
                panic!("glitching detected");
            }
        }
    }}.into()
}

#[proc_macro]
pub fn rand_ops(input: TokenStream) -> TokenStream {
    let rng = parse_macro_input!(input as Expr);

    let num_ops_xor: u32 = OsRng.gen();
    let n_xor: u32 = OsRng.gen();

    let random_xor: u32 = OsRng.gen();
    let random_shift: u32 = OsRng.gen_range(0..32);
    let random_divide: u32 = OsRng.gen();

    let possible_rand_ops = [
        quote! { check = core::hint::black_box(check ^ i) },
        quote! { check = core::hint::black_box(check.wrapping_add(i)) },
        quote! { check = core::hint::black_box(check.wrapping_mul(i)) },
        quote! { check = core::hint::black_box(!(check ^ i)) },
        quote! { check = core::hint::black_box(!check) },
        quote! { check = core::hint::black_box(check ^ #random_xor) },
        quote! { check = core::hint::black_box((check / #random_divide) ^ (check % #random_divide))},
        quote! { check = core::hint::black_box(check.rotate_right(#random_shift)) },
        quote! { check = core::hint::black_box(#rng.next_u32() ^ i) },
        quote! { check = core::hint::black_box(#rng.next_u32() ^ check) },
    ];

    let num_ops: u32 = OsRng.gen_range(5..16);

    let mut ops = proc_macro2::TokenStream::new();
    for i in 0..num_ops {
        // TODO: maybe make duplicate ops have different constants
        let op = possible_rand_ops.choose(&mut OsRng).unwrap();

        ops.extend(quote! {
            #i => #op,
        });
    }

    quote! {{
        use design_utils::anti_hardware::RngCore;
        let num_ops = core::hint::black_box((#rng.next_u32() ^ #num_ops_xor) % 500);

        let mut check = core::hint::black_box(#rng.next_u32() ^ #n_xor);
        for i in 0..num_ops {
            match core::hint::black_box(check % #num_ops) {
                #ops
                _ => panic!("impossible check"),
            }
        }
    }}
    .into()
}
