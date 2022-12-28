// Strict encoding library for deterministic binary serialization.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2019-2023 by
//     Dr. Maxim Orlovsky <orlovsky@ubideco.org>
//
// Copyright 2022-2023 UBIDECO Institute
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::any;
use std::collections::BTreeSet;
use std::fmt::{Debug, Display};

use crate::{FieldName, LibName, TypeName};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error)]
#[display("unexpected variant {1} for enum or union {0}")]
pub struct VariantError<V: Debug + Display>(TypeName, V);

pub trait StrictDumb: Sized {
    fn strict_dumb() -> Self;
}

impl<T> StrictDumb for T
where T: StrictType + Default
{
    fn strict_dumb() -> T { T::default() }
}

pub trait StrictType: Sized {
    const STRICT_LIB_NAME: &'static str;
    fn strict_name() -> Option<String> {
        fn get_ident(path: &str) -> &str { path.rsplit_once("::").map(|(_, n)| n).unwrap_or(path) }

        let name = any::type_name::<Self>();
        let (base, generics) = name.split_once("<").unwrap_or((name, ""));
        let generics = generics.trim_end_matches('>');
        let mut ident = get_ident(base).to_owned();
        for arg in generics.split(',') {
            ident.push('_');
            ident.extend(get_ident(arg).chars());
        }
        Some(ident)
    }
}

impl<T: StrictType> StrictType for &T {
    const STRICT_LIB_NAME: &'static str = T::STRICT_LIB_NAME;
}

pub trait StrictProduct: StrictType + StrictDumb {}

pub trait StrictTuple: StrictProduct {
    const ALL_FIELDS: &'static [u8];
    fn strict_check_fields() {
        let name = Self::strict_name().unwrap_or_else(|| s!("<unnamed>"));
        assert!(
            !Self::ALL_FIELDS.is_empty(),
            "tuple type {} does not contain a single field defined",
            name
        );
        let mut set = BTreeSet::<u8>::new();
        set.extend(Self::ALL_FIELDS);
        assert_eq!(
            set.len(),
            Self::ALL_FIELDS.len(),
            "tuple type {} contains repeated field ids",
            name
        );
    }

    fn strict_type_info() -> TypeInfo<Self> {
        Self::strict_check_fields();
        TypeInfo {
            lib: libname!(Self::STRICT_LIB_NAME),
            name: Self::strict_name().map(|name| tn!(name)),
            cls: TypeClass::Tuple(Self::ALL_FIELDS),
            dumb: Self::strict_dumb(),
        }
    }
}

pub trait StrictStruct: StrictProduct {
    const ALL_FIELDS: &'static [(u8, &'static str)];

    fn strict_check_fields() {
        let name = Self::strict_name().unwrap_or_else(|| s!("<unnamed>"));
        assert!(
            !Self::ALL_FIELDS.is_empty(),
            "struct type {} does not contain a single field defined",
            name
        );
        let (ords, names): (BTreeSet<_>, BTreeSet<_>) = Self::ALL_FIELDS.iter().copied().unzip();
        assert_eq!(
            ords.len(),
            Self::ALL_FIELDS.len(),
            "struct type {} contains repeated field ids",
            name
        );
        assert_eq!(
            names.len(),
            Self::ALL_FIELDS.len(),
            "struct type {} contains repeated field names",
            name
        );
    }

    fn strict_type_info() -> TypeInfo<Self> {
        Self::strict_check_fields();
        TypeInfo {
            lib: libname!(Self::STRICT_LIB_NAME),
            name: Self::strict_name().map(|name| tn!(name)),
            cls: TypeClass::Struct(Self::ALL_FIELDS),
            dumb: Self::strict_dumb(),
        }
    }
}

pub trait StrictSum: StrictType {
    const ALL_VARIANTS: &'static [(u8, &'static str)];

    fn strict_check_variants() {
        let name = Self::strict_name().unwrap_or_else(|| s!("<unnamed>"));
        assert!(
            !Self::ALL_VARIANTS.is_empty(),
            "type {} does not contain a single variant defined",
            name
        );
        let (ords, names): (BTreeSet<_>, BTreeSet<_>) = Self::ALL_VARIANTS.iter().copied().unzip();
        assert_eq!(
            ords.len(),
            Self::ALL_VARIANTS.len(),
            "type {} contains repeated variant ids",
            name
        );
        assert_eq!(
            names.len(),
            Self::ALL_VARIANTS.len(),
            "type {} contains repeated variant names",
            name
        );
    }

    fn variant_ord(&self) -> u8 {
        let variant = self.variant_name();
        for (ord, name) in Self::ALL_VARIANTS {
            if *name == variant {
                return *ord;
            }
        }
        unreachable!(
            "not all variants are enumerated for {} enum in StrictUnion::all_variants \
             implementation",
            any::type_name::<Self>()
        )
    }
    fn variant_name(&self) -> &'static str;
}

pub trait StrictUnion: StrictSum + StrictDumb {
    fn strict_type_info() -> TypeInfo<Self> {
        Self::strict_check_variants();
        TypeInfo {
            lib: libname!(Self::STRICT_LIB_NAME),
            name: Self::strict_name().map(|name| tn!(name)),
            cls: TypeClass::Union(Self::ALL_VARIANTS),
            dumb: Self::strict_dumb(),
        }
    }
}

pub trait StrictEnum
where
    Self: StrictSum + Copy + TryFrom<u8, Error = VariantError<u8>>,
    u8: From<Self>,
{
    fn from_variant_name(name: &FieldName) -> Result<Self, VariantError<&FieldName>>;

    fn strict_type_info() -> TypeInfo<Self> {
        Self::strict_check_variants();
        TypeInfo {
            lib: libname!(Self::STRICT_LIB_NAME),
            name: Self::strict_name().map(|name| tn!(name)),
            cls: TypeClass::Enum(Self::ALL_VARIANTS),
            dumb: Self::try_from(Self::ALL_VARIANTS[0].0)
                .expect("first variant contains invalid value"),
        }
    }
}

pub enum TypeClass {
    Embedded,
    Enum(&'static [(u8, &'static str)]),
    Union(&'static [(u8, &'static str)]),
    Tuple(&'static [u8]),
    Struct(&'static [(u8, &'static str)]),
}

pub struct TypeInfo<T: StrictType> {
    lib: LibName,
    name: Option<TypeName>,
    cls: TypeClass,
    dumb: T,
}
