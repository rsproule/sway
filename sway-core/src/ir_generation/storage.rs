use crate::asm_generation::from_ir::ir_type_size_in_bytes;
use fuel_crypto::Hasher;
use fuel_types::{Bytes32, Bytes8};
use sway_ir::{
    constant::{Constant, ConstantValue},
    context::Context,
    irtype::{AggregateContent, Type},
};
use sway_types::{state::StateIndex, JsonStorageInitializers, StorageInitializer};

/// Hands out storage keys using a state index and a list of subfield indices.
/// Basically returns sha256("storage_<state_index>_<idx1>_<idx2>_..")
///
pub(super) fn get_storage_key<T>(ix: &StateIndex, indices: &[T]) -> Bytes32
where
    T: std::fmt::Display,
{
    Hasher::hash(indices.iter().fold(
        format!(
            "{}{}",
            sway_utils::constants::STORAGE_DOMAIN_SEPARATOR,
            ix.to_usize()
        ),
        |acc, i| format!("{}_{}", acc, i),
    ))
}

use uint::construct_uint;

#[allow(
// These two warnings are generated by the `construct_uint!()` macro below.
    clippy::assign_op_pattern,
    clippy::ptr_offset_with_cast
)]
pub(super) fn add_to_b256(x: fuel_types::Bytes32, y: u64) -> fuel_types::Bytes32 {
    construct_uint! {
        struct U256(4);
    }
    let x = U256::from(*x);
    let y = U256::from(y);
    let res: [u8; 32] = (x + y).into();
    fuel_types::Bytes32::from(res)
}

/// Given a constant value `constant`, a type `ty`, a state index, and a vector of subfield
/// indices, serialize the constant into a vector of storage initializers. The keys (slots) are
/// generated using the state index and the subfield indices which are recursively built. The
/// values are generated such that each subfield gets its own storage slot except for enums and
/// strings which are spread over successive storage slots (use `serialize_to_words` in this case).
///
/// This behavior matches the behavior of how storage slots are assigned for storage reads and
/// writes (i.e. how `state_read_*` and `state_write_*` instructions are generated).
///
pub fn serialize_to_storage_initializers(
    constant: &Constant,
    context: &Context,
    ix: &StateIndex,
    ty: &Type,
    indices: &[usize],
) -> JsonStorageInitializers {
    match (&ty, &constant.value) {
        (_, ConstantValue::Undef) => vec![],
        (Type::Unit, ConstantValue::Unit) => vec![StorageInitializer {
            slot: get_storage_key(ix, indices),
            value: Bytes32::new([0; 32]),
        }],
        (Type::Bool, ConstantValue::Bool(b)) => {
            vec![StorageInitializer {
                slot: get_storage_key(ix, indices),
                value: Bytes32::new(
                    [0; 31]
                        .iter()
                        .cloned()
                        .chain([if *b { 0x01 } else { 0x00 }].iter().cloned())
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap(),
                ),
            }]
        }
        (Type::Uint(_), ConstantValue::Uint(n)) => {
            vec![StorageInitializer {
                slot: get_storage_key(ix, indices),
                value: Bytes32::new(
                    [0; 24]
                        .iter()
                        .cloned()
                        .chain(n.to_be_bytes().iter().cloned())
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap(),
                ),
            }]
        }
        (Type::B256, ConstantValue::B256(b)) => {
            vec![StorageInitializer {
                slot: get_storage_key(ix, indices),
                value: Bytes32::new(*b),
            }]
        }
        (Type::Array(_), ConstantValue::Array(_a)) => {
            unimplemented!("Arrays in storage have not been implemented yet.")
        }
        (Type::Struct(aggregate), ConstantValue::Struct(vec)) => {
            match &context.aggregates[aggregate.0] {
                AggregateContent::FieldTypes(field_tys) => vec
                    .iter()
                    .zip(field_tys.iter())
                    .enumerate()
                    .flat_map(|(i, (f, ty))| {
                        serialize_to_storage_initializers(
                            f,
                            context,
                            ix,
                            ty,
                            &indices
                                .iter()
                                .cloned()
                                .chain(vec![i].iter().cloned())
                                .collect::<Vec<usize>>(),
                        )
                    })
                    .collect(),
                _ => unreachable!("Wrong content for struct."),
            }
        }
        (Type::Union(_), _) | (Type::String(_), _) => {
            // Serialize the constant data in words and add zero words until the number of words
            // is a multiple of 4. This is useful because each storage slot is 4 words.
            let mut packed = serialize_to_words(constant, context, ty);
            packed.extend(vec![
                Bytes8::new([0; 8]);
                ((packed.len() + 3) / 4) * 4 - packed.len()
            ]);

            assert!(packed.len() % 4 == 0);

            // Return a list of StorageInitializers
            // First get the keys then get the values
            (0..(ir_type_size_in_bytes(context, ty) + 31) / 32)
                .into_iter()
                .map(|i| add_to_b256(get_storage_key(ix, indices), i))
                .zip((0..packed.len() / 4).into_iter().map(|i| {
                    Bytes32::new(
                        Vec::from_iter((0..4).into_iter().flat_map(|j| *packed[4 * i + j]))
                            .try_into()
                            .unwrap(),
                    )
                }))
                .map(|(k, r)| StorageInitializer { slot: k, value: r })
                .collect()
        }
        _ => vec![],
    }
}

/// Given a constant value `constant` and a type `ty`, serialize the constant into a vector of
/// words and add left padding up to size of `ty`.
///
pub fn serialize_to_words(constant: &Constant, context: &Context, ty: &Type) -> Vec<Bytes8> {
    match (&ty, &constant.value) {
        (_, ConstantValue::Undef) => vec![],
        (Type::Unit, ConstantValue::Unit) => vec![Bytes8::new([0; 8])],
        (Type::Bool, ConstantValue::Bool(b)) => {
            vec![Bytes8::new(
                [0; 7]
                    .iter()
                    .cloned()
                    .chain([if *b { 0x01 } else { 0x00 }].iter().cloned())
                    .collect::<Vec<u8>>()
                    .try_into()
                    .unwrap(),
            )]
        }
        (Type::Uint(_), ConstantValue::Uint(n)) => {
            vec![Bytes8::new(n.to_be_bytes())]
        }
        (Type::B256, ConstantValue::B256(b)) => Vec::from_iter(
            (0..4)
                .into_iter()
                .map(|i| Bytes8::new(b[8 * i..8 * i + 8].try_into().unwrap())),
        ),
        (Type::String(_), ConstantValue::String(s)) => {
            // Turn the serialized words (Bytes8) into seriliazed storage slots (Bytes32)
            // Pad to word alignment
            let mut s = s.clone();
            s.extend(vec![0; ((s.len() + 3) / 4) * 4 - s.len()]);

            assert!(s.len() % 8 == 0);

            // Group into words
            Vec::from_iter((0..s.len() / 8).into_iter().map(|i| {
                Bytes8::new(
                    Vec::from_iter((0..8).into_iter().map(|j| s[8 * i + j]))
                        .try_into()
                        .unwrap(),
                )
            }))
        }
        (Type::Array(_), ConstantValue::Array(_)) => {
            unimplemented!("Arrays in storage have not been implemented yet.")
        }
        (Type::Struct(aggregate), ConstantValue::Struct(vec)) => {
            match &context.aggregates[aggregate.0] {
                AggregateContent::FieldTypes(field_tys) => vec
                    .iter()
                    .zip(field_tys.iter())
                    .flat_map(|(f, ty)| serialize_to_words(f, context, ty))
                    .collect(),
                _ => unreachable!("Wrong content for struct."),
            }
        }
        (Type::Union(_), _) => {
            let value_size_in_words = ir_type_size_in_bytes(context, ty) / 8;
            let constant_size_in_words = ir_type_size_in_bytes(context, &constant.ty) / 8;
            assert!(value_size_in_words >= constant_size_in_words);

            // Add enough left padding to satisfy the actual size of the union
            let padding_size_in_words = value_size_in_words - constant_size_in_words;
            vec![Bytes8::new([0; 8]); padding_size_in_words as usize]
                .iter()
                .cloned()
                .chain(
                    serialize_to_words(constant, context, &constant.ty)
                        .iter()
                        .cloned(),
                )
                .collect()
        }
        _ => vec![],
    }
}
