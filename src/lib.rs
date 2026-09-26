//! Общая «обвязка» курса. Живёт в lib, а не в модулях, чтобы:
//!   * каждый `moduleNN.rs` оставался файлом *про язык*, а не про тестирование;
//!   * было видно, как работает разделение lib / bin (Модуль 10);
//!   * `macro_rules!` экспортировался наружу честно (Модуль 14).

pub mod alloc;
pub mod harness;
pub mod rng;

pub use harness::{bench, report, Bench, Report};

// ─────────────────────────────────────────────────────────────────────────────
// Макрос-конструктор битовых флагов. Пишем вручную, чтобы увидеть, что
// делает `#[derive]` в настоящем крейте `bitflags`.
//
// ВАЖНО (и это ровно то, за что любят Rust): флаги — это **не**
// перечисление, а число. Флаговые комбинации не перечислить в enum,
// поэтому в C++ приходится городить перегрузки операторов над `enum class`,
// а в Rust достаточно того, что тип **новый**:
// `MyFlags + MyFlags` не существует, `MyFlags | u32` не существует,
// а `u32` нельзя случайно передать туда, где ждут флаги.
// ─────────────────────────────────────────────────────────────────────────────

/// Определяет новый тип-новьтап поверх целого: флаговые операции,
/// но без смешивания с «голым» числом.
#[macro_export]
macro_rules! bitflags {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident : $ty:ty {
            $( $(#[$fmeta:meta])* $flag:ident = $value:expr ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis struct $name($ty);

        impl $name {
            $( $(#[$fmeta])* $vis const $flag: Self = Self($value); )*

            /// Пустой набор флагов.
            $vis const NONE: Self = Self(0);

            #[inline]
            $vis const fn bits(self) -> $ty { self.0 }

            #[inline]
            $vis const fn from_bits(bits: $ty) -> Self { Self(bits) }

            /// Есть ли **все** флаги из `other`.
            #[inline]
            $vis const fn contains(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }

            /// Есть ли **хотя бы один** флаг из `other`.
            #[inline]
            $vis const fn intersects(self, other: Self) -> bool {
                (self.0 & other.0) != 0
            }

            #[inline]
            $vis const fn is_empty(self) -> bool { self.0 == 0 }

            /// Включает флаги из `other`.
            #[inline]
            $vis const fn with(self, other: Self) -> Self { Self(self.0 | other.0) }

            /// Выключает флаги из `other`.
            #[inline]
            $vis const fn without(self, other: Self) -> Self { Self(self.0 & !other.0) }

            /// Если установлен `flag` — включить, иначе выключить.
            #[inline]
            $vis const fn set(self, flag: Self, on: bool) -> Self {
                if on { self.with(flag) } else { self.without(flag) }
            }
        }

        impl ::std::ops::BitOr for $name {
            type Output = Self;
            #[inline]
            fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
        }
        impl ::std::ops::BitAnd for $name {
            type Output = Self;
            #[inline]
            fn bitand(self, rhs: Self) -> Self { Self(self.0 & rhs.0) }
        }
        impl ::std::ops::BitXor for $name {
            type Output = Self;
            #[inline]
            fn bitxor(self, rhs: Self) -> Self { Self(self.0 ^ rhs.0) }
        }
        impl ::std::ops::Not for $name {
            type Output = Self;
            #[inline]
            fn not(self) -> Self { Self(!self.0) }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, concat!(stringify!($name), "(0x{:x})"), self.0)
            }
        }
    };
}
