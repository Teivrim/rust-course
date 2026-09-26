//! ============================================================================
//! МОДУЛЬ 12 — UNSAFE И FFI
//! ============================================================================
//!
//! ЧАСТЬ 12.1 — ТЕОРИЯ: что делает `unsafe`
//! ─────────────────────────────────────────────────────────────────────────────
//! `unsafe` НЕ отключает проверки. Он говорит: «здесь я беру
//! ответственность на инварианты, которые компилятор проверить не может».
//! Если инвариант нарушен — это UB (undefined behavior), а НЕ паника.
//! Самая опасная часть: UB компилятор оптимизирует ПО-ТИХУ, программа
//! работает «неправильно», и найти причину невозможно.
//!
//! ПЯТЬ ОПЕРАЦИЙ, ТРЕБУЮЩИХ `unsafe` (и ничего больше):
//!
//!   1. Разыменование СЫРОГО указателя (`*const T`, `*mut T`).
//!      `&T`/`&mut T` разыменовывать безопасно — они уже проверены.
//!   2. Вызов `unsafe fn`.
//!   3. Доступ к изменяемой статической переменной.
//!   4. Реализация `unsafe impl` (Send/Sync/Unpin/...).
//!   5. Объявление `unsafe` блока FFI (`unsafe extern`).
//!
//! `unsafe fn` vs `unsafe {}`:
//!   * `unsafe fn` — функция, которую МОЖНО вызвать только из unsafe.
//!     Тело тоже считается unsafe (в edition 2024 внутри тела всё равно
//!     нужен явный `unsafe {}` — проверка не отключена).
//!   * `unsafe {}` — блок в безопасном коде: «здесь я осознанно
//!     нарушаю/исполняю контракт».
//!
//! ⚠ `unsafe impl Send` — самый опасный из пяти. Он говорит компилятору
//!   «этот тип можно передать в другой поток». Если в нём есть `Rc` или
//!   `Cell`, получишь data race, а компилятор тебе поверит.
//!
//! ПРАВИЛО РАБОТЫ (применяй буквально):
//!   1. `unsafe` — только в маленьком, узком месте.
//!   2. Снаружи — безопасная обёртка с проверками.
//!   3. Перед каждым `unsafe` пиши `// SAFETY:` с тремя пунктами:
//!      корректность указателя, корректность длины, корректность выравнивания.
//!   4. Проверяй инварианты в debug-режиме (обычные `assert!`),
//!      даже если в release их нет.
//!
//! ЧЕМ ЛОВИТЬ ОШИБКИ:
//!   * `debug_assertions` — ловит часть контрактов.
//!   * ASAN/TSAN — не входят в стандартную поставку.
//!   * `cargo +nightly miri test` — интерпретатор, проверяющий UB.
//!     ⚠ Miri не запустит FFI-код и ассемблер, но проверяет lifetime,
//!     алиасинг, выравнивание, out-of-bounds. Для этого курса — лучший
//!     инструмент после компилятора.
//!   * Sanitizer недели (`-Zsanitizer=address`) — для нового кода.
//!
//! `unsafe impl` для FFI-типов: любой тип, пришедший из C, по умолчанию
//! НЕ Send и НЕ Sync (он содержит сырые указатели). Это не паранойя —
//! C-структура может содержать что угодно. Если ты знаешь, что твой
//! FFI-тип безопасен, пиши `unsafe impl Send` с обоснованием.

#![allow(unused_variables, unused_imports, dead_code)]
// Трансмут здесь — ПЕДАГОГИЧЕСКИЙ приём: показать, как он выглядит.
// Линт предупреждает, что есть безопасная замена, и это верно — но
// понимать разницу обязательно, прежде чем пользоваться заменой.
#![allow(unnecessary_transmutes)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

use std::ffi::c_void;
use std::ptr::NonNull;

fn main() {
    harness::module(12, "Unsafe и FFI");

    part_12_1(); // сырые указатели и безопасная обёртка
    part_12_2(); // FFI к Windows API — настоящая
    part_12_3(); // MaybeUninit, срезы из сырой памяти, transmute

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("12.1  RAII-обёртка над C-указателем с Drop", task_12_1);
    r.task("12.2  FFI-биндинги: вызов Win32 + проверка раскладки", task_12_2);
    r.task("12.3  Типизированный срез поверх чужого буфера", task_12_3);
    r.summary(12);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 12.1 — ПРИМЕР: сырой указатель и безопасная обёртка
// ─────────────────────────────────────────────────────────────────────────────
fn part_12_1() {
    harness::part("12.1", "ПРИМЕР: сырой указатель, NonNull, безопасная обёртка");

    // --- Что именно требует unsafe -----------------------------------------
    let mut value = 42u32;
    let ptr: *mut u32 = &mut value;

    // SAFETY: ptr получен из &mut value, значит указывает на живую
    // память, выровненную как u32, и единственную ссылку на неё
    // (заём закончится, когда ptr перестанет использоваться).
    let read_back = unsafe { *ptr };
    println!("  *ptr = {read_back} (чтение через сырой указатель)");

    // SAFETY: тот же указатель, единственный владелец памяти.
    unsafe { *ptr = 100 };
    println!("  после *ptr = 100: value = {value}");

    // ⚠ НЕЛЬЗЯ: держать сырой указатель, пока живна ссылка.
    //   let r = &value;
    //   let p: *mut u32 = &mut value;  // ⛔ E0502 — r всё ещё жив
    //   Без borrow checker это дало бы aliasing и UB.

    // --- NonNull: указатель, который не бывает null -------------------------
    // NonNull — это newtype над сырым указателем без niche-дырки.
    // Из-за этого `Option<NonNull<T>>` занимает РОВНО 8 байт.
    let nn: NonNull<u32> = NonNull::new(&mut value).expect("&mut никогда не null");
    println!("  NonNull: {:p}, Option<NonNull<u32>> = {} байт",
        nn.as_ptr(), size_of::<Option<NonNull<u32>>>());
    // SAFETY: nn получен из &mut value, значение живо до конца блока.
    unsafe { nn.as_ptr().write(7) };
    println!("  после nn.as_ptr().write(7): {value}");

    // --- НЕБЕЗОПАСНАЯ обёртка: принимает C-указатель -----------------------
    let leaked: *mut i32 = Box::into_raw(Box::new(7i32));
    // SAFETY: указатель получен из Box::into_raw, длина 4 соответствует массиву,
    // владение переходит к ForeignBuffer, Drop вернёт его в Box ровно один раз.
    let raw = unsafe { ForeignBuffer::from_raw(leaked, 4) };
    println!("  ForeignBuffer: len={}, first={}", raw.len(), raw.first());
    drop(raw); // Drop вернёт память обратно аллокатору
    println!("  Drop отработал, двойного освобождения нет");

    // --- copy_nonoverlapping vs copy ----------------------------------------
    let src = [1u8, 2, 3, 4];
    let mut dst = [0u8; 4];
    // SAFETY: src и dst не пересекаются, длины равны 4, выравнивание u8 = 1.
    unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), 4) };
    println!("  copy_nonoverlapping: {src:?} -> {dst:?}");

    // ⚠ copy (а не copy_nonoverlapping) допускает пересечение.
    //   ⚠ НО: если источник и назначение — ОДНО и то же место (полностью),
    //      поведение не определено. Пересечение по ИНТЕРВАЛАМ допустимо
    //      только как memmove, а не как mempcopy.
}

/// Обёртка над памятью, полученной из C. `Drop` обязателен —
// иначе будет утечка. Все `unsafe` спрятаны внутрь.
struct ForeignBuffer {
    ptr: NonNull<i32>,
    len: usize,
}

impl ForeignBuffer {
    /// Принимает указатель из C. Проверяет, что он не null и len > 0.
    /// # Safety
    /// Вызывающий обязан передать указатель, полученный от malloc-подобного
    /// аллокатора, с len элементами, и не освобождать его сам.
    unsafe fn from_raw(ptr: *mut i32, len: usize) -> Self {
        // SAFETY (внутри unsafe fn): мы проверяем null и len,
        // поэтому нижеразыменование корректно.
        assert!(!ptr.is_null(), "C передал null");
        assert!(len > 0, "нулевая длина");
        ForeignBuffer { ptr: NonNull::new(ptr).expect("проверено выше"), len }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn first(&self) -> i32 {
        // SAFETY: память выровнена как i32 (экспортировано из i32),
        // живёт не меньше self.len элементов, и у нас единственный
        // владелец (Box::into_raw отдал владение нам).
        unsafe { *self.ptr.as_ptr() }
    }
}

impl Drop for ForeignBuffer {
    fn drop(&mut self) {
        // SAFETY: указатель получен из Box::into_raw, значит его можно
        // вернуть обратно в Box, и Drop вызовется ровно один раз.
        // ⚠ ВАЖНО: Drop НЕ ДОЛЖЕН ПАНИКАТЬ. Иначе double panic при
        //   раскрутке → abort без раскрутки → утечка всего стека.
        let _b = unsafe { Box::from_raw(self.ptr.as_ptr()) };
    }
}

use std::mem::size_of;

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 12.2 — ТЕОРИЯ: FFI
// ─────────────────────────────────────────────────────────────────────────────
//
// ДЛЯ ЧЕГО FFI В ДВИЖКЕ: любой вызов в Vulkan, OpenGL, stb_image,
// PhysX — это FFI. API Vulkan — это 800+ C-функций, и `ash` — просто
// сгенерированные биндинги. Но биндинги можно написать руками.
//
// ПРАВИЛА FFI, НАРУШЕНИЕ КОТОРЫХ = UB:
//
//   1. `#[repr(C)]` на КАЖДОЙ структуре, которая пересекает границу.
//      Без него компилятор переставляет поля, и C увидит мусор.
//   2. Типы примитивов — из `std::ffi` / `core::ffi`:
//      c_char, c_int, c_uint, c_long, c_void, c_float, c_double.
//      ⚠ `bool` в Rust — 1 байт, в C++ — тоже, но в C (до C23) —
//      ЛЮБОЕ ненулевое значение. Не передавай `bool` в C.
//   3. Строки — только с `\0`. `&str` не подходит: он не NUL-терминирован.
//      Используй `CString` / `OsStr` + `encode_wide` для Windows.
//   4. `extern "system"` для Windows (это stdcall/cdecl по конвенции
//      платформы), `extern "C"` для остального.
//   5. Out-параметры — обычные `*mut T`.
//   6. Callback-и — `extern "system" fn(...)`, и они НЕ могут паниковать
//      (паника через FFI-границу = UB, а не «ошибка»).
//
// ЗАГРУЗКА БИБЛИОТЕК — почему Vulkan грузится динамически:
// на машине может быть любой драйвер, любой API (vulkan-1.dll), и
// библиотека обязана грузиться ПОСЛЕ выбора драйвера. Поэтому в Rust
// нет `#[link(name = "vulkan-1")]`, а есть `LoadLibraryW` +
// `GetProcAddress`.
//
// `#[link(name = "kernel32", kind = "raw-dylib")]` — это «ленивая»
// линковка: символы не ищутся при сборке, а подставляются загрузчиком
// при запуске. Работает без import-библиотек — что важно, когда Visual
// Studio не установлена (как в этом окружении).

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct SystemTime {
    year: i16,
    month: u16,
    _pad0: u16, // ⚠ явный padding — в C он есть, и repr(C) его сохранит
    day: u8,
    _pad1: u8,
    weekday: u16,
}

#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    fn GetModuleHandleW(lpmodulename: *const u16) -> *mut c_void;
    fn GetSystemTime(lpsystemtime: *mut SystemTime) -> *mut c_void;
    fn GetTickCount64() -> u64;
    fn GetCurrentProcessId() -> u32;
    fn IsDebuggerPresent() -> i32;
}

fn part_12_2() {
    harness::part("12.2", "ПРИМЕР: FFI к Windows API — по-настоящему");

    unsafe {
        // --- Указатель на уже загруженный модуль ----------------------------
        // Строки в Windows — UTF-16 с NUL. Конвертируем правильно.
        let name = wide_nul("kernel32.dll");
        let handle = GetModuleHandleW(name.as_ptr());
        println!("  GetModuleHandleW(\"kernel32.dll\") = {handle:?}");
        assert!(!handle.is_null(), "kernel32 не может не загрузиться");

        // --- Out-параметр: структуру заполняет C ----------------------------
        let mut t = SystemTime::default();
        GetSystemTime(&mut t);
        println!("  GetSystemTime = {:04}-{:02}-{:02} (weekday {})", t.year, t.month, t.day, t.weekday);
        println!("    size_of::<SystemTime>() = {} — проверь против sizeof в C",
            size_of::<SystemTime>());

        // --- Возвращаемое значение примитива --------------------------------
        println!("  GetCurrentProcessId() = {}", GetCurrentProcessId());
        println!("  IsDebuggerPresent()   = {} (0 = отладчика нет)", IsDebuggerPresent());

        // --- Замер: 10 мс в GetTickCount64 -----------------------------------
        let t0 = GetTickCount64();
        let mut acc = 0u64;
        for i in 0..5_000_000u64 {
            acc = acc.wrapping_add(i);
        }
        let elapsed = GetTickCount64() - t0;
        std::hint::black_box(acc);
        println!("  5M итераций заняли {elapsed} мс (высчитал сам Windows)");
    }

    // --- Вызов из безопасного кода ------------------------------------------
    // Сверни unsafe в маленькую безопасную функцию — и весь TUI приложения
    // про FFI не знает.
    println!("  безопасная обёртка uptime_ms() = {}", uptime_ms());
    println!("  безопасная обёртка in_debugger() = {}", in_debugger());
}

/// UTF-16 строка с завершающим NUL — единственный правильный способ
/// передать строку в Win32.
fn wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Безопасная обёртка: unsafe не протекает наружу.
fn uptime_ms() -> u64 {
    // SAFETY: функция без аргументов и без указателей, паник не содержит.
    unsafe { GetTickCount64() }
}

fn in_debugger() -> bool {
    // SAFETY: как выше. ⚠ ВНИМАНИЕ: C возвращает "любое ненулевое" как true.
    //   Поэтому НЕльзя `unsafe { IsDebuggerPresent() }` вернуть как bool
    //   напрямую по FFI-сигнатуре Rust — здесь корректное сравнение.
    unsafe { IsDebuggerPresent() != 0 }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 12.3 — ТЕОРИЯ: MaybeUninit, срезы, transmute
// ─────────────────────────────────────────────────────────────────────────────
//
// `MaybeUninit<T>` — «возможно, инициализированное значение».
// Нужен, потому что `let x: T;` требует, чтобы x уже имел значение,
// а буфер памяти из malloc/C этого не гарантирует.
//
// ГЛАВНЫЙ ПРИНЦИП: UB возникает НЕ при создании мусора, а при ЧТЕНИИ
// неинициализированной памяти. `MaybeUninit` позволяет создать мусор
// и запрещает его прочитать.
//
// ОПЕРАЦИИ:
//   MaybeUninit::uninit()      — мусор
//   MaybeUninit::zeroed()      — нули (⚠ валидно не для всех T!)
//   MaybeUninit::new(v)        — готовое значение
//   .assume_init()             → T, «я обещаю, что значение есть»
//   ptr.write(v)               — записать, вернуть &mut T (безопасно!)
//   ptr.read()                 — скопировать и оставить uninit
//
// ⚠ `MaybeUninit::zeroed()` — ловушка. Для `bool`, `char`, `&T`,
//   `NonZeroU32`, `enum` без варианта 0 это UB или «висячий» указатель.
//
// СОЗДАНИЕ СРЕЗА ИЗ СЫРОЙ ПАМЯТИ:
//
//   unsafe { slice::from_raw_parts(ptr, len) }
//
// Требования (каждое — причина UB при нарушении):
//   1. ptr выровнен под T.
//   2. ptr валиден для чтения len элементов T.
//   3. Все len элементов проинициализированы.
//   4. Память не изменяется снаружи, пока жив срез (&mut — эксклюзивно).
//
// TRANSMUTE: меняет раскладку, не меняя размер.
//   `transmute::<[u8; 4], u32>` — читает байты как число.
//   ⚠ Порядок байтов определяется платформой. На x86 это LE.
//   ⚠ НИКОГДА не делай transmute между `[u8; 4]` и `&str` или
//     между структурами разного размера (это UB, а не паника).
//
// БЕЗОПАСНАЯ альтернатива transmute: `u32::from_ne_bytes([u8; 4])`.
// Она проверяет размер на этапе компиляции и читает в заданном
// порядке байтов. В движке всегда выбирай её, когда возможно.

fn part_12_3() {
    harness::part("12.3", "ПРИМЕР: MaybeUninit, слайсы, transmute");

    // --- MaybeUninit -------------------------------------------------------
    let mut uninit: std::mem::MaybeUninit<u32> = std::mem::MaybeUninit::uninit();
    // ⚠ прочитать uninit нельзя — только записать:
    uninit.write(1234);
    // SAFETY: мы только что записали значение через write().
    let value = unsafe { uninit.assume_init() };
    println!("  MaybeUninit: записали 1234, прочитали {value}");

    let zeroed = std::mem::MaybeUninit::<u8>::zeroed();
    // SAFETY: u8 корректно представляется нулём.
    assert_eq!(unsafe { zeroed.assume_init() }, 0);
    println!("  MaybeUninit::zeroed::<u8>() = 0 (для u8 безопасно)");

    // ⚠ Для bool zeroed — НЕЛЬЗЯ: 0 невалидно.
    // let bad = std::mem::MaybeUninit::<bool>::zeroed();
    // unsafe { bad.assume_init() }  // UB!

    // --- Срез из массива: БЕЗ unsafe, если массива достаточно ---------------
    let arr = [1u32, 2, 3, 4, 5];
    let slice: &[u32] = &arr[1..4];
    println!("  безопасный срез: {slice:?}");

    // --- Срез из Vec: тоже без unsafe --------------------------------------
    let v = vec![10u32, 20, 30];
    let s: &[u32] = &v[..];
    println!("  срез из Vec: {s:?}");

    // --- Срез из Vec<u8> к f32: БЕЗОПАСНО, если выровнено ------------------
    // ⚠ Это небезопасно только если Vec не выровнен под f32. Vec<u8>
    //   выровнен как u8 (align 1), а f32 требует 4. На практике malloc
    //   всегда возвращает выровненную память, но ФОРМАЛЬНО это UB.
    //   Поэтому ниже — правильный вариант с проверкой.
    let raw_bytes: Vec<u8> = 1.5f32.to_le_bytes().to_vec();
    if let Some(f) = bytes_to_f32(&raw_bytes) {
        println!("  bytes_to_f32({raw_bytes:?}) = {f} (align проверен)");
    }
    // А вот «просто reinterpret» — работает на практике, но не формально:
    let unchecked = f32::from_le_bytes([raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3]]);
    println!("  f32::from_le_bytes = {unchecked} (безопасная альтернатива transmute)");

    // --- transmute ----------------------------------------------------------
    let bytes: [u8; 4] = 0x3F80_0000u32.to_le_bytes();
    // ⚠ Порядок байтов платформенный! На x86 это корректно.
    let f: f32 = unsafe { std::mem::transmute(bytes) };
    println!("  transmute([u8;4] -> f32) = {f} (⚠ endian-зависимо!)");

    // --- read_unaligned: когда выравнивание не гарантировано ----------------
    // Это ТО ЖЕ, что C++ `memcpy` в int — единственный безопасный
    // способ прочитать невыровненное число.
    let storage = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let mid = 1usize; // невыровненное смещение для u32
    let p = unsafe { storage.as_ptr().add(mid) as *const u8 };
    let v = unsafe { std::ptr::read_unaligned(p as *const u32) };
    println!("  read_unaligned по смещению {mid} = 0x{v:08x} (допустимо при невыравнивании)");
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 12.4 — ТЕОРИЯ: как НЕ надо и как надо
// ─────────────────────────────────────────────────────────────────────────────
//
// ❌ ПЛОХО: `unsafe` размазан по приложению.
//    fn render() { unsafe { vkCreateShaderModule(p, ..) } }   // 40 вызовов
//
// ✅ ХОРОШО: весь FFI в одном модуле `ffi/`, наружу — безопасные типы.
//    mod ffi { unsafe { fn create_shader_module(..) -> Result<..> } }
//    pub mod render { use super::ffi; }   // ни одного unsafe
//
// ТАК И ДОЛЖНО ВЫГЛЯДЕТЬ `Main/src` в настоящем движке:
//
//   src/
//     main.rs
//     app.rs        — цикл, состояние, ошибки (0 байт unsafe)
//     scene.rs      — данные сцены (0 unsafe)
//     math.rs       — матрицы (0 unsafe)
//     ffi/          — ВЕСЬ unsafe, ни разу не выходит наружу
//       mod.rs
//       vk_types.rs  — repr(C) структуры Vulkan
//       vk_sys.rs    — extern "system" объявления
//       vk_load.rs   — LoadLibraryW / GetProcAddress
//       wrapper.rs   — RAII-обёртки (модуль 18)
//     resources.rs  — AssetManager (0 unsafe)

fn part_12_4() {
    harness::part("12.4", "ПРИМЕР: считаем unsafe в модуле");

    // Простой эксперимент: сколько unsafe-блоков в этом файле.
    // Всё, что ниже — безопасный код, который зовёт unsafe-обёртку.
    let b = ForeignBuffer::from_raw_checked(Box::into_raw(Box::new(1i32)), 1);
    println!("  безопасный вызов unsafe-обёртки: first = {}", b.first());
    drop(b);

    // ⚠ from_raw — unsafe fn, поэтому вызывать его можно только из unsafe.
    //   from_raw_checked — безопасная версия с проверками. Разница понятна.
}

impl ForeignBuffer {
    /// Безопасная версия: проверяет всё, что может проверить Rust.
    /// # Panics
    /// Если ptr == null или len == 0.
    fn from_raw_checked(ptr: *mut i32, len: usize) -> Self {
        assert!(!ptr.is_null(), "C передал null");
        assert!(len > 0, "нулевая длина");
        // SAFETY: мы только что проверили ptr и len, поэтому все
        // инварианты from_raw выполнены.
        unsafe { Self::from_raw(ptr, len) }
    }
}

/// Безопасная конверсия &[u8] -> Option<f32> с проверкой длины.
/// Реальный проект берёт `bytemuck` и проверяет выравнивание.
fn bytes_to_f32(b: &[u8]) -> Option<f32> {
    if b.len() != 4 {
        return None;
    }
    Some(f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 12.5 — ЗАДАНИЕ 12.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// RAII-обёртка над «C-буфером». Это прообраз `VkBuffer` из модуля 18,
/// поэтому сделай сразу правильно.
///
/// ```ignore
/// struct CBuffer {
///     ptr: NonNull<u8>,
///     len: usize,
///     device: u64,          // «владелец» (например, VkDevice)
/// }
///
/// impl CBuffer {
///     unsafe fn from_raw(ptr: *mut u8, len: usize, device: u64) -> Option<Self>;
///     fn as_slice(&self) -> &[u8];
///     fn as_mut_slice(&mut self) -> &mut [u8];
///     fn device(&self) -> u64;
///     fn into_raw(self) -> (*mut u8, u64);   // «отдать» владение назад
/// }
///
/// impl Drop for CBuffer { ... }
/// ```
///
/// Требования:
///   * `from_raw` — `unsafe fn` (нельзя гарантировать, что указатель
///     от C валиден), но возвращает `Option`: null → None.
///   * `as_slice` — `unsafe { slice::from_raw_parts(..) }` с комментарием
///     SAFETY из трёх пунктов.
///   * `into_raw` — НЕ вызывает `Drop`. Используй `ManuallyDrop` или
///     `mem::forget`. ⚠ И объясни в комментарии, почему `mem::forget`
///     здесь корректен (в отличие от `delete this` в C++).
///   * `Drop` — НЕ паникует ни при каких условиях.
///   * Реализуй `unsafe impl Send for CBuffer` — устройство можно
///     передавать между потоками. ⚠ И напиши в комментарии, почему
///     это допустимо и что было бы недопустимо.
fn task_12_1() {
    use std::mem::ManuallyDrop;

    struct CBuffer {
        ptr: NonNull<u8>,
        len: usize,
        device: u64,
    }

    impl CBuffer {
        unsafe fn from_raw(ptr: *mut u8, len: usize, device: u64) -> Option<Self> {
            not_yet!("NonNull::new(ptr)? и верни Self");
        }

        fn as_slice(&self) -> &[u8] {
            not_yet!("unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) } + SAFETY");
        }

        fn as_mut_slice(&mut self) -> &mut [u8] {
            not_yet!("то же через from_raw_parts_mut");
        }

        fn device(&self) -> u64 {
            self.device
        }

        fn into_raw(self) -> (*mut u8, u64) {
            not_yet!("ManuallyDrop: забери поля и не вызывай Drop");
        }
    }

    impl Drop for CBuffer {
        fn drop(&mut self) {
            not_yet!("имитируем free() — но НЕ паникуй, иначе double panic при раскрутке");
        }
    }

    // SAFETY: буфер — обычная память из Vec, u8 выровнен как 1,
    // Drop вызывается ровно один раз.
    let mut raw: Vec<u8> = (0..16u8).collect();
    let (ptr, len) = (raw.as_mut_ptr(), raw.len());
    let dev = 0xDEAD_BEEF;

    let mut buf = unsafe { CBuffer::from_raw(ptr, len, dev) }.expect("ptr не null");
    assert_eq!(buf.as_slice().len(), 16);
    assert_eq!(buf.as_slice()[0], 0);
    assert_eq!(buf.device(), 0xDEAD_BEEF);
    buf.as_mut_slice()[0] = 99;
    assert_eq!(raw[0], 99, "видно исходный Vec");

    // ownership переходит обратно — Vec корректно освободит память.
    let (_p, _d) = buf.into_raw();
    drop(raw);
    assert!(unsafe { CBuffer::from_raw(std::ptr::null_mut(), 0, 0) }.is_none());
}

/// ЧАСТЬ 12.6 — ЗАДАНИЕ 12.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Настоящий FFI. Объяви структуру так, как она лежит в C, проверь
/// размер, вызови Win32-функцию и верни типизированный результат.
///
/// ```ignore
/// #[repr(C)]
/// #[derive(Debug, Default, Clone, Copy)]
/// struct FileTime { low: u32, high: u32 }        // 8 байт
///
/// #[link(name = "kernel32", kind = "raw-dylib")]
/// unsafe extern "system" {
///     fn GetFileTime(h: *mut c_void, out: *mut FileTime) -> i32;
///     fn GetCurrentThreadId() -> u32;
///     fn GetTickCount() -> u32;                    // 32-битный, максимум 49 дней
/// }
///
/// fn file_time_now() -> Option<u64>;
/// fn thread_id() -> u32;
/// fn ticks_u32() -> u32;
/// ```
///
/// Требования:
///   * `file_time_now` возвращает `None`, если вызов не удался (0).
///     Формат FILETIME: 100-наносекундных интервалов с 1601-01-01,
///     поэтому это НЕ unix-время. Не вычитай «секунды от начала эпохи
///     Unix» — объясни в комментарии правильную формулу константы
///     (11644473600 секунд).
///   * `ticks_u32` — и покажи в комментарии ловушку переполнения через
///     49.7 дней и чем плох `GetTickCount` против `GetTickCount64`.
///   * Проверь `size_of::<FileTime>() == 8` assert'ом.
///   * Все `unsafe` — внутри этих трёх функций, не снаружи.
fn task_12_2() {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Debug, Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32", kind = "raw-dylib")]
    unsafe extern "system" {
        fn GetFileTime(h: *mut c_void, out: *mut FileTime) -> i32;
        fn GetCurrentThreadId() -> u32;
        fn GetTickCount() -> u32;
    }

    fn file_time_now() -> Option<u64> {
        not_yet!("GetFileTime(null(), &mut ft); если 0 -> None, иначе (hi<<32|lo) как u64");
    }

    fn thread_id() -> u32 {
        not_yet!("безопасная обёртка над GetCurrentThreadId");
    }

    fn ticks_u32() -> u32 {
        not_yet!("безопасная обёртка над GetTickCount");
    }

    assert_eq!(size_of::<FileTime>(), 8, "два u32 рядом — 8 байт");
    let ft = file_time_now().expect("вызов прошёл");
    assert!(ft > 0, "FILETIME ненулевой");
    // Unix-время = (FILETIME / 10_000_000) - 11_644_473_600
    let unix = (ft / 10_000_000).saturating_sub(11_644_473_600);
    println!("  FILETIME = {ft} → unix ≈ {unix} (должно быть около 1.7-1.9 млрд)");
    assert!(unix > 1_600_000_000, "эпоха не сходится: {unix}");

    println!("  GetCurrentThreadId() = {}", thread_id());
    println!("  GetTickCount()       = {} мс", ticks_u32());
    println!("  ⚠ GetTickCount переполняется через 49.7 дня — считай разницу в u32");
}

/// ЧАСТЬ 12.7 — ЗАДАНИЕ 12.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Типизированный срез поверх чужого буфера — то, что в реальном
/// проекте делает `bytemuck::cast_slice`, но здесь ты пишешь это сам
/// и поэтому понимаешь, что внутри.
///
/// ```ignore
/// fn as_f32_slice(bytes: &[u8]) -> Result<&[f32], CastError>;
/// fn as_vertex_slice(bytes: &[u8]) -> Result<&[Vertex], CastError>;
/// fn to_le_bytes_f32(v: &[f32]) -> Vec<u8>;
/// ```
///
/// Требования:
///   * НИКАКОГО `transmute` и никаких `unsafe` в `as_f32_slice`:
///     валидация длины через `chunks_exact(4)`, проверка выравнивания
///     через `align_of::<f32>()` и `as_ptr() as usize % 4 == 0`.
//   * `as_vertex_slice` возвращает срез ТОЛЬКО если длина кратна
//     размеру вершины и указатель выровнен.
//   * `CastError` — enum с двумя вариантами (BadLen, BadAlign).
//   * ⚠ Сформулируй в комментарии, ПОЧЕМУ безопасной функции
//     недостаточно: если выравнивание не совпало, безопасный код
//     НЕ МОЖЕТ сделать `&[f32]` из `&[u8]` вообще. Что тогда делают
//     в реальном проекте? (Ответ: копируют в выровненный Vec,
//     либо используют bytemuck, который делает то же через unsafe
//     внутри — и требует, чтобы ты ДОКАЗАЛ выравнивание снаружи.)
///   * `to_le_bytes_f32` — безопасная, через flat_map.
fn task_12_3() {
    #[derive(Debug, PartialEq)]
    enum CastError {
        BadLen { got: usize, need: usize },
        BadAlign { addr: usize, need: usize },
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Vertex {
        pos: [f32; 3],
        uv: [f32; 2],
    }

    fn as_f32_slice(bytes: &[u8]) -> Result<&[f32], CastError> {
        not_yet!("chunks_exact(4) + проверка align + from_le_bytes на каждый");
    }

    fn as_vertex_slice(bytes: &[u8]) -> Result<&[Vertex], CastError> {
        not_yet!("сначала приведи к &[f32], потом transmute-подобная сборка через copy");
    }

    fn to_le_bytes_f32(v: &[f32]) -> Vec<u8> {
        not_yet!("flat_map(|f| f.to_le_bytes())");
    }

    let floats = [1.0f32, 2.0, 3.0, -4.0];
    let bytes = to_le_bytes_f32(&floats);
    assert_eq!(bytes.len(), 16);
    let back = as_f32_slice(&bytes).expect("выровнено и длина кратна 4");
    assert_eq!(back, &floats);

    assert!(matches!(as_f32_slice(&[1, 2, 3]), Err(CastError::BadLen { .. })));

    // Вершины
    let v = Vertex { pos: [1.0, 2.0, 3.0], uv: [0.5, 0.5] };
    let vbytes: Vec<u8> = to_le_bytes_f32(&[v.pos[0], v.pos[1], v.pos[2], v.uv[0], v.uv[1]]);
    let verts = as_vertex_slice(&vbytes).expect("20 байт, выровнено");
    assert_eq!(verts.len(), 1);
    assert_eq!(verts[0].pos, [1.0, 2.0, 3.0]);

    assert!(as_vertex_slice(&[0u8; 3]).is_err());
    println!("  as_f32_slice / as_vertex_slice без единого unsafe");
}