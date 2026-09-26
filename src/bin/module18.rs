//! ============================================================================
//! МОДУЛЬ 18 — VULKAN НА RUST
//! ============================================================================
//!
//! ЧАСТЬ 18.1 — ТЕОРИЯ: почему Vulkan грузится динамически
//! ─────────────────────────────────────────────────────────────────────────────
//! Vulkan НЕ линкуется. Причина архитектурная: на одной машине может
//! быть несколько драйверов, а выбрать загрузчик нужно после выбора
//! драйвера (ICD — Installable Client Driver). Поэтому:
//!
//!   1. `LoadLibraryW("vulkan-1.dll")` — загружать сам API Vulkan
//!      (он всегда есть в системе, если установлен драйвер);
//!   2. `vkGetInstanceProcAddr(instance, name)` — получить указатель
//!      на функцию;
//!   3. вызвать её через `transmute` на `extern "system" fn(...)`.
//!
//! ПОСЛЕ СОЗДАНИЯ `VkInstance` доступна ВСЯ функциональность. До него —
//! только пять глобальных функций (`vkGetInstanceProcAddr`,
//! `vkCreateInstance`, `vkEnumerateInstanceExtensionProperties`,
//! `vkEnumerateInstanceLayerProperties`, и `vkEnumerateInstanceVersion`
//! в 1.1+).
//!
//! `ash` делает ровно это и добавляет типобезопасность поверх. Здесь мы
//! делаем это руками — и это единственный способ понять, что внутри
//! `ash` (а именно там живёт весь «магический» код, который новички
//! принимают за чудо).
//!
//! ЧТО ТАКОЕ sType и ЗАЧЕМ ОН. Каждая структура Vulkan начинается с
//! `sType: VkStructureType` — числового кода её типа и `pNext`, через
//! который передаются расширенные структуры. Драйвер читает `sType`
//! и понимает, что пришло. Ошибка в sType = «мусор на входе»,
//! и в C++ это проходит молча. В Rust — то же, но ты пишешь число
//! руками, и компилятор не поможет. Именно поэтому в `ash` есть
//! макрос `struct_tys!` и константы с именами.
//!
//! ⚠ ВАЖНО ПРО РАСКЛАДКУ. Все структуры — `#[repr(C)]`, и порядок
//!   полей СОВПАДАЕТ с заголовком. Строки в Vulkan — массивы `char`
//!   фиксированного размера (`char name[256]`), то есть обычные
//!   `[u8; 256]`, а не `&str` и не `CString`.

#![allow(unused_variables, unused_imports, dead_code)]
// Заглушки not_yet! расходились: они возвращают тип never, поэтому всё,
// что стоит после них, выглядит как недостижимый код. Это ожидаемо.
#![allow(unreachable_code, unused_mut)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

use std::ffi::c_void;
use std::ptr;

fn main() {
    harness::module(18, "Vulkan на Rust");

    part_18_1(); // загрузка DLL, запрос функций, перечисление слоёв и расширений
    part_18_2(); // создание и уничтожение VkInstance
    part_18_3(); // RAII-обёртки и порядок уничтожения

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("18.1  Расширенный загрузчик: все пять глобальных функций", task_18_1);
    r.task("18.2  RAII-обёртки: Instance/Device/Buffer с правильным порядком", task_18_2);
    r.task("18.3  Builder-паттерн для структур Vulkan", task_18_3);
    r.summary(18);
}

// ─────────────────────────────────────────────────────────────────────────────
// ОБЪЯВЛЕНИЯ FFI
// ─────────────────────────────────────────────────────────────────────────────

// Загрузка DLL. `raw-dylib` = символы подставляются загрузчиком при
// старте, линковщик их не ищет. Это работает даже без Visual Studio,
// чего не делает обычный `#[link(name = "kernel32")]`.
#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    fn LoadLibraryW(lpfilename: *const u16) -> *mut c_void;
    fn FreeLibrary(hmodule: *mut c_void) -> i32;
    fn GetProcAddress(hmodule: *mut c_void, lpprocname: *const u8) -> *mut c_void;
}

// ── Константы Vulkan ────────────────────────────────────────────────────────

/// `VK_STRUCTURE_TYPE_APPLICATION_INFO`
const VK_STRUCTURE_TYPE_APPLICATION_INFO: i32 = 0;
/// `VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO`
const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: i32 = 1;

/// `VK_SUCCESS`
const VK_SUCCESS: i32 = 0;
/// `VK_INCOMPLETE` — список был обрезан, надо вызвать ещё раз
const VK_INCOMPLETE: i32 = 1;
/// `VK_ERROR_OUT_OF_HOST_MEMORY`
const VK_ERROR_OUT_OF_HOST_MEMORY: i32 = -1;
/// `VK_ERROR_INITIALIZATION_FAILED`
const VK_ERROR_INITIALIZATION_FAILED: i32 = -3;

/// `VK_MAX_EXTENSION_NAME_SIZE` и `VK_MAX_DESCRIPTION_SIZE`
const VK_MAX_EXTENSION_NAME_SIZE: usize = 256;
const VK_MAX_DESCRIPTION_SIZE: usize = 256;

/// `VK_API_VERSION_1_0` = VK_MAKE_API_VERSION(0, 1, 0, 0)
const fn vk_make_api_version(variant: u32, major: u32, minor: u32, patch: u32) -> u32 {
    (variant << 29) | (major << 22) | (minor << 12) | patch
}

// ── Структуры Vulkan ────────────────────────────────────────────────────────

/// `VK_MAX_PHYSICAL_DEVICE_NAME_SIZE`
const VK_MAX_PHYSICAL_DEVICE_NAME_SIZE: usize = 256;

/// `VkApplicationInfo` — обязательный первый аргумент `pApplicationInfo`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VkApplicationInfo {
    s_type: i32,
    p_next: *const c_void,
    p_application_name: *const u8, // *const c_char
    application_version: u32,
    p_engine_name: *const u8,
    engine_version: u32,
    api_version: u32,
}

impl VkApplicationInfo {
    const fn new(app: *const u8, app_ver: u32, engine: *const u8, eng_ver: u32, api: u32) -> Self {
        Self {
            s_type: VK_STRUCTURE_TYPE_APPLICATION_INFO,
            p_next: ptr::null(),
            p_application_name: app,
            application_version: app_ver,
            p_engine_name: engine,
            engine_version: eng_ver,
            api_version: api,
        }
    }
}

/// `VkInstanceCreateInfo`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VkInstanceCreateInfo {
    s_type: i32,
    p_next: *const c_void,
    /// Флаги: `VkInstanceCreateFlags` = u32
    flags: u32,
    p_application_info: *const VkApplicationInfo,
    enabled_layer_count: u32,
    pp_enabled_layer_names: *const *const u8,
    enabled_extension_count: u32,
    pp_enabled_extension_names: *const *const u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VkExtensionProperties {
    extension_name: [u8; VK_MAX_EXTENSION_NAME_SIZE],
    spec_version: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VkLayerProperties {
    layer_name: [u8; VK_MAX_EXTENSION_NAME_SIZE],
    spec_version: u32,
    implementation_version: u32,
    description: [u8; VK_MAX_DESCRIPTION_SIZE],
}

/// Дескрипторы Vulkan — ПОЛНЫЕ УКАЗАТЕЛИ на C-объекты.
type VkInstance = *mut c_void;

/// ── Глобальные функции Vulkan ──────────────────────────────────────────────

type PfnVoidFunction = *mut c_void;
type PfnGetInstanceProcAddr = unsafe extern "system" fn(*mut c_void, *const u8) -> PfnVoidFunction;
type PfnEnumerateInstanceExtensionProperties =
    unsafe extern "system" fn(*const c_void, *mut u32, *mut VkExtensionProperties) -> i32;
type PfnEnumerateInstanceLayerProperties =
    unsafe extern "system" fn(*mut u32, *mut VkLayerProperties) -> i32;
type PfnEnumerateInstanceVersion = unsafe extern "system" fn(*mut u32) -> i32;
type PfnCreateInstance =
    unsafe extern "system" fn(*const VkInstanceCreateInfo, *const c_void, *mut VkInstance) -> i32;
type PfnDestroyInstance = unsafe extern "system" fn(*mut c_void, *const c_void);

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 18.1 — ЗАГРУЗЧИК
// ─────────────────────────────────────────────────────────────────────────────

/// Типобезопасная обёртка над указателем на функцию Vulkan.
///
/// ГЛАВНАЯ МЫСЛЬ МОДУЛЯ: `GetProcAddress` возвращает `*mut c_void`, а
/// вызвать через него можно только `transmute`. Ошибка в сигнатуре —
/// UB, и компилятор её не поймает: он верит нам.
///
/// ЛЕЧЕНИЕ (именно так сделано в `ash`): хранить не голый
/// `*mut c_void`, а структуру с полем-функцией нужного типа. Тогда
/// `transmute` происходит ОДИН РАЗ, в методе `load`, и сигнатура
/// берётся из объявления поля, а не из того, что мы написали в
/// `transmute`. Ошибка в сигнатуре уезжает в другое место кода,
/// где хотя бы видна в диффе.
struct VkFunction<T> {
    inner: T,
}

impl<T> VkFunction<T> {
    /// # Safety
    /// `f` должен указывать на функцию с сигнатурой `T`.
    unsafe fn from_raw(f: PfnVoidFunction) -> Option<Self> {
        if f.is_null() {
            return None;
        }
        // ⚠ ПОЧЕМУ НЕ `transmute`: размеры типов-указателей на функцию
        //   и на данные компилятор не считает одинаковыми (в
        //   некоторых ABI различаются), и `transmute` откажется.
        //   `transmute_copy` копирует байты и проверяет, что в
        //   источник помещается столько же, сколько занимает T.
        //   Для указателей на функции на всех реальных платформах
        //   это верно, и мы дополнительно проверяем это в debug.
        debug_assert!(
            std::mem::size_of::<T>() <= std::mem::size_of::<PfnVoidFunction>(),
            "тип функции больше, чем указатель: трансмутить нельзя"
        );
        // SAFETY: вызывающий гарантирует, что `f` — функция типа T.
        //   Размеры проверены выше (в release — верно по построению).
        let inner: T = unsafe { std::mem::transmute_copy(&f) };
        Some(VkFunction { inner })
    }
}

/// Загруженная библиотека Vulkan.
struct VulkanLib {
    /// Ссылка на DLL. Держим, пока нужны функции из неё.
    _module: *mut c_void,
    get_proc_addr: VkFunction<PfnGetInstanceProcAddr>,
    enumerate_ext: VkFunction<PfnEnumerateInstanceExtensionProperties>,
    enumerate_layers: VkFunction<PfnEnumerateInstanceLayerProperties>,
    create: VkFunction<PfnCreateInstance>,
    // vkEnumerateInstanceVersion есть только в Vulkan 1.1+
    enumerate_version: Option<VkFunction<PfnEnumerateInstanceVersion>>,
}

impl VulkanLib {
    /// Шаг 1: загрузить DLL и получить глобальные функции.
    fn load(name: &str) -> Result<Self, VkLoadError> {
        // SAFETY: `name` — валидная UTF-16 с NUL, действие безопасно.
        let module = unsafe { LoadLibraryW(wide_nul(name).as_ptr()) };
        if module.is_null() {
            return Err(VkLoadError::LibraryNotFound(name.to_string()));
        }
        // SAFETY: модуль действителен, имя — ASCII с NUL.
        unsafe {
            let gpa = GetProcAddress(module, b"vkGetInstanceProcAddr\0".as_ptr());
            if gpa.is_null() {
                FreeLibrary(module);
                return Err(VkLoadError::SymbolMissing("vkGetInstanceProcAddr".into()));
            }
            let gpa: PfnGetInstanceProcAddr = std::mem::transmute_copy(&gpa);

            // ⚠ С NULL-инстансом через vkGetInstanceProcAddr доступны
            //   ТОЛЬКО ГЛОБАЛЬНЫЕ команды. Даже `vkDestroyInstance`
            //   штатный Windows-загрузчик с NULL НЕ отдаёт — его берут
            //   уже по созданному инстансу. На этом падают первые
            //   попытки написать загрузчик, и ошибка выглядит как
            //   «нет драйвера», хотя драйвер есть.
            let ext = gpa(ptr::null_mut(), b"vkEnumerateInstanceExtensionProperties\0".as_ptr());
            let layers = gpa(ptr::null_mut(), b"vkEnumerateInstanceLayerProperties\0".as_ptr());
            let create = gpa(ptr::null_mut(), b"vkCreateInstance\0".as_ptr());
            // vkEnumerateInstanceVersion может отсутствовать на 1.0 —
            // это НЕ ошибка, а признак старого драйвера.
            let ver = gpa(ptr::null_mut(), b"vkEnumerateInstanceVersion\0".as_ptr());

            let enumerate_ext = VkFunction::from_raw(ext).ok_or_else(|| {
                VkLoadError::SymbolMissing("vkEnumerateInstanceExtensionProperties".into())
            })?;
            let enumerate_layers = VkFunction::from_raw(layers).ok_or_else(|| {
                VkLoadError::SymbolMissing("vkEnumerateInstanceLayerProperties".into())
            })?;
            let create = VkFunction::from_raw(create)
                .ok_or_else(|| VkLoadError::SymbolMissing("vkCreateInstance".into()))?;

            Ok(VulkanLib {
                _module: module,
                get_proc_addr: VkFunction { inner: gpa },
                enumerate_ext,
                enumerate_layers,
                create,
                enumerate_version: VkFunction::from_raw(ver),
            })
        }
    }

    /// Строка с NUL-terminated именем.
    fn instance_extensions(&self) -> Result<Vec<String>, VkResult> {
        // ДВА ПРОХОДА — так устроен весь Vulkan API:
        // 1) count = nullptr → узнаём количество;
        // 2) выделяем массив и заполняем.
        // ⚠ Если между проходами драйвер изменился, второй вернёт
        //   VK_INCOMPLETE — это нормальная ситуация, не ошибка.
        unsafe {
            let mut count = 0u32;
            let r = (self.enumerate_ext.inner)(ptr::null(), &mut count, ptr::null_mut());
            if r != VK_SUCCESS {
                return Err(VkResult::from(r));
            }
            let mut buf = vec![
                VkExtensionProperties { extension_name: [0; VK_MAX_EXTENSION_NAME_SIZE], spec_version: 0 };
                count as usize
            ];
            let r = (self.enumerate_ext.inner)(ptr::null(), &mut count, buf.as_mut_ptr());
            if r != VK_SUCCESS && r != VK_INCOMPLETE {
                return Err(VkResult::from(r));
            }
            buf.truncate(count as usize);
            Ok(buf.iter().map(|e| cstr_to_string(&e.extension_name)).collect())
        }
    }

    fn instance_layers(&self) -> Result<Vec<String>, VkResult> {
        unsafe {
            let mut count = 0u32;
            let r = (self.enumerate_layers.inner)(&mut count, ptr::null_mut());
            if r != VK_SUCCESS {
                return Err(VkResult::from(r));
            }
            let mut buf = vec![
                VkLayerProperties {
                    layer_name: [0; VK_MAX_EXTENSION_NAME_SIZE],
                    spec_version: 0,
                    implementation_version: 0,
                    description: [0; VK_MAX_DESCRIPTION_SIZE],
                };
                count as usize
            ];
            let r = (self.enumerate_layers.inner)(&mut count, buf.as_mut_ptr());
            if r != VK_SUCCESS && r != VK_INCOMPLETE {
                return Err(VkResult::from(r));
            }
            buf.truncate(count as usize);
            Ok(buf.iter().map(|l| cstr_to_string(&l.layer_name)).collect())
        }
    }

    /// Версия API. `None` → драйвер 1.0 (функции нет).
    fn api_version(&self) -> Option<u32> {
        let f = self.enumerate_version.as_ref()?;
        let mut v = 0u32;
        // SAFETY: указатель на u32 валиден, функция его заполнит.
        let r = unsafe { (f.inner)(&mut v) };
        if r == VK_SUCCESS { Some(v) } else { None }
    }

    /// Достать функцию для конкретного инстанса.
    fn instance_fn<T>(&self, instance: *mut c_void, name: &[u8]) -> Option<VkFunction<T>> {
        // SAFETY: имя — ASCII с NUL, instance получен из Vulkan.
        let p = unsafe { (self.get_proc_addr.inner)(instance, name.as_ptr()) };
        // SAFETY: вызывающий обязан знать, что функция имеет тип T.
        unsafe { VkFunction::from_raw(p) }
    }
}

impl Drop for VulkanLib {
    fn drop(&mut self) {
        // ⚠ Порядок важен: сначала убеждаемся, что все VkInstance
        //   уже уничтожены (это гарантирует их Drop), и только потом
        //   выгружаем DLL. Если бы порядок был обратный, вызов
        //   vkDestroyInstance после FreeLibrary был бы прыжок в
        //   освобождённую память.
        // SAFETY: модуль загружен нами, выгружаем ровно один раз.
        unsafe { FreeLibrary(self._module) };
    }
}

#[derive(Debug, PartialEq)]
enum VkLoadError {
    LibraryNotFound(String),
    SymbolMissing(String),
}

impl std::fmt::Display for VkLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VkLoadError::LibraryNotFound(n) => write!(f, "не найден {n}. Установи Vulkan Runtime / драйвер"),
            VkLoadError::SymbolMissing(s) => write!(f, "нет символа {s}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VkResult(i32);

impl VkResult {
    fn from(code: i32) -> Self {
        VkResult(code)
    }
    fn ok(self) -> bool {
        self.0 >= 0
    }
}

impl std::fmt::Display for VkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            0 => "VK_SUCCESS",
            1 => "VK_INCOMPLETE",
            -1 => "VK_ERROR_OUT_OF_HOST_MEMORY",
            -2 => "VK_ERROR_OUT_OF_DEVICE_MEMORY",
            -3 => "VK_ERROR_INITIALIZATION_FAILED",
            -4 => "VK_ERROR_DEVICE_LOST",
            -5 => "VK_ERROR_MEMORY_MAP_FAILED",
            -6 => "VK_ERROR_LAYER_NOT_PRESENT",
            -7 => "VK_ERROR_EXTENSION_NOT_PRESENT",
            -9 => "VK_ERROR_INCOMPATIBLE_DRIVER",
            other => return write!(f, "VkResult({other})"),
        };
        write!(f, "{s}")
    }
}

impl std::error::Error for VkResult {}

fn part_18_1() {
    harness::part("18.1", "ПРИМЕР: загрузка vulkan-1.dll и запрос функций");

    // --- Загрузка -----------------------------------------------------------
    let lib = match VulkanLib::load("vulkan-1.dll") {
        Ok(l) => l,
        Err(e) => {
            println!("  {e}");
            println!("    Установи Vulkan Runtime (https://vulkan.lunarg.com) и повтори.");
            println!("    Валидатор расширений: {}", std::env::var("VULKAN_SDK").unwrap_or_default());
            harness::part("18.2", "ПРИМЕР: создание VkInstance");
            println!("  пропускаем: без драйвера создавать нечего");
            harness::part("18.3", "ПРИМЕР: RAII-обёртки");
            println!("  пропускаем");
            return;
        }
    };
    println!("  vulkan-1.dll загружена");

    // --- Версия API ---------------------------------------------------------
    match lib.api_version() {
        Some(v) => {
            let major = (v >> 22) & 0x7F;
            let minor = (v >> 12) & 0x3FF;
            let patch = v & 0xFFF;
            println!("  vkEnumerateInstanceVersion: {major}.{minor}.{patch} (0x{v:08x})");
            assert!(major >= 1, "ожидали Vulkan 1.x");
        }
        None => println!("  vkEnumerateInstanceVersion отсутствует → драйвер Vulkan 1.0"),
    }

    // --- Слои ---------------------------------------------------------------
    match lib.instance_layers() {
        Ok(layers) => {
            println!("  слоёв (layers): {}", layers.len());
            for l in layers.iter().take(6) {
                println!("    - {l}");
            }
            // Валидатор — слой, а не расширение. Это важно: новички
            // ищут VK_LAYER_KHRONOS_validation среди расширений.
            if layers.iter().any(|l| l == "VK_LAYER_KHRONOS_validation") {
                println!("    ✓ валидатор доступен — включай его при разработке");
            } else {
                println!("    ✗ валидатор не установлен (нужен vulkan-sdk / sdkmanager)");
            }
        }
        Err(e) => println!("  слои: ошибка {e}"),
    }

    // --- Расширения ---------------------------------------------------------
    match lib.instance_extensions() {
        Ok(exts) => {
            println!("  расширений уровня instance: {}", exts.len());
            let interesting: Vec<&String> = exts
                .iter()
                .filter(|e| e.contains("surface") || e.contains("debug_utils") || e.contains("portability"))
                .collect();
            for e in interesting {
                println!("    * {e}");
            }
            // ⚠ VK_KHR_surface требует окна (surface). Без окна его
            //   включение бессмысленно, но API его не проверит.
            println!("    ⚠ VK_KHR_surface бесполезен без окна — но валидатор не предупредит");
        }
        Err(e) => println!("  расширения: ошибка {e}"),
    }

    // --- Свойства структуры: проверяем раскладку ----------------------------
    // ⚠ ЗДЕСЬ БЫЛА ОШИБКА, КОТОРУЮ Я СДЕЛАЛ САМ: я написал
    //   «VkApplicationInfo = 56 байт». На самом деле 48. Разбор:
    //
    //   смещение  поле                 размер
    //   0         sType                4
    //   4         (padding)            4   <- pNext требует выравнивания 8
    //   8         pNext                8
    //   16        pApplicationName     8
    //   24        applicationVersion   4
    //   28        (padding)            4   <- pEngineName требует 8
    //   32        pEngineName          8
    //   40        engineVersion        4
    //   44        apiVersion           4
    //   ---------------------------------------
    //   итого 48, выравнивание 8
    //
    // Вывод: Rust и C сошлись, и это не совпадение — #[repr(C)]
    // гарантирует одинаковую раскладку. Но ГАРАНТИЯ работает только
    // при #[repr(C)]: без него компилятор вправе переставить поля
    // «покрасивее» и всё разъедется. Это ровно тот случай, где
    // ошибка не падает, а молча портит данные.
    println!("\n  раскладка структур (x64):");
    println!("    size_of::<VkApplicationInfo>()     = {}", size_of::<VkApplicationInfo>());
    println!("    size_of::<VkInstanceCreateInfo>()  = {}", size_of::<VkInstanceCreateInfo>());
    println!("    align_of::<VkInstanceCreateInfo>() = {}", align_of::<VkInstanceCreateInfo>());
    println!("    size_of::<VkExtensionProperties>() = {}", size_of::<VkExtensionProperties>());
    assert_eq!(size_of::<VkApplicationInfo>(), 48, "48 = 4+4pad+8+8+4+4pad+8+4+4");
    assert_eq!(size_of::<VkInstanceCreateInfo>(), 64, "64, см. разбор в комментарии");
    assert_eq!(size_of::<VkExtensionProperties>(), 260, "char[256] + u32");
    println!("    ✓ совпадает с C. #[repr(C)] работает — padding на месте.");

    // Показываем реальные смещения полей: берём адрес поля, вычитаем базу.
    // Это БЕЗОПАСНО: мы не создаём ссылку на невыровненную память.
    let app = VkApplicationInfo::new(ptr::null(), 0, ptr::null(), 0, 0);
    let base = &app as *const VkApplicationInfo as usize;
    println!("    смещения полей VkApplicationInfo:");
    println!("      s_type              @{}", (&app.s_type as *const _ as usize) - base);
    println!("      p_next              @{}", (&app.p_next as *const _ as usize) - base);
    println!("      p_application_name  @{}", (&app.p_application_name as *const _ as usize) - base);
    println!("      application_version @{}", (&app.application_version as *const _ as usize) - base);
    println!("      p_engine_name       @{}", (&app.p_engine_name as *const _ as usize) - base);
    println!("      engine_version      @{}", (&app.engine_version as *const _ as usize) - base);
    println!("      api_version         @{}", (&app.api_version as *const _ as usize) - base);

    // Дальше части 18.2 и 18.3 вызываются из main.
    let _ = lib;
}

use std::mem::{align_of, size_of};

/// UTF-16 строка с NUL. Единственный правильный способ для Win32.
fn wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// C-строка из массива байт (обрезает по NUL).
fn cstr_to_string(b: &[u8]) -> String {
    let end = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 18.2 — ТЕОРИЯ: создание инстанса
// ─────────────────────────────────────────────────────────────────────────────
//
// `vkCreateInstance` принимает УКАЗАТЕЛИ на структуры. В Rust для этого
// есть три подхода, и выбор между ними — отдельная инженерная задача:
//
//   1. Стековые локальные переменные + `&raw const`. Дёшево, но
//      нужно вручную следить за временем жизни.
//   2. Builder-паттерн: накапливаем поля в структуре, в конце
//      собираем `repr(C)`-структуру и передаём указатель.
//   3. `ash::vk::InstanceCreateInfo::builder()` — то же, что (2),
//      сгенерировано макросом.
//
// ЗАЧЕМ НУЖЕН `application_name` и `engine_name`. Это ТЕЛЕМЕТРИЯ:
// драйвер и слои видят, какое приложение работает. Многие счётчики
// производительности (если у пользователя включён оверлей) показывают
// эти имена. Указывать `appName` — вежливость и диагностика.
//
// `apiVersion`: если поставить 0, драйвер сам выберет 1.0 (не надо!).
// Ставь явно. И помни: `VK_API_VERSION_1_1` в значении 0 означает
// «не запрашиваю конкретную версию» — ловушка новичков.
//
// ⚠ ВАЛИДАТОР. Если слой `VK_LAYER_KHRONOS_validation` доступен,
//   ВКЛЮЧАЙ его только в debug-сборке. В release он даёт +30-100%
//   к времени кадра. Стандартная схема:
//
// ```ignore
// #[cfg(debug_assertions)] const USE_VALIDATION: bool = true;
// #[cfg(not(debug_assertions))] const USE_VALIDATION: bool = false;
// ```

/// RAII-обёртка `VkInstance`. `Drop` уничтожает инстанс.
struct Instance {
    handle: VkInstance,
    /// ⚠ `vkDestroyInstance` получают ПО СОЗДАННОМУ инстансу, а не
    ///   с NULL. Поэтому указатель на функцию хранится РЯДОМ с
    ///   инстансом: иначе после выгрузки DLL он станет висящим и
    ///   Drop упадёт с segfault вместо нормального освобождения.
    destroy: VkFunction<PfnDestroyInstance>,
    api_version: u32,
}

impl Instance {
    fn create(
        lib: &VulkanLib,
        app_name: &str,
        layers: &[&str],
        extensions: &[&str],
    ) -> Result<Self, InstanceError> {
        // Шаг 1: строки. Vulkan хочет `const char*` массив указателей
        // на C-строки. Строки должны ЖИТЬ до конца вызова.
        let app_c = nul_bytes(app_name);
        let engine_c = nul_bytes("novell-engine");

        let layer_c: Vec<Vec<u8>> = layers.iter().map(|s| nul_bytes(s)).collect();
        let layer_ptrs: Vec<*const u8> = layer_c.iter().map(|v| v.as_ptr()).collect();

        let ext_c: Vec<Vec<u8>> = extensions.iter().map(|s| nul_bytes(s)).collect();
        let ext_ptrs: Vec<*const u8> = ext_c.iter().map(|v| v.as_ptr()).collect();

        let app_info = VkApplicationInfo::new(
            app_c.as_ptr(),
            vk_make_api_version(0, 1, 0, 0),
            engine_c.as_ptr(),
            vk_make_api_version(0, 1, 0, 0),
            vk_make_api_version(0, 1, 0, 0),
        );

        let create_info = VkInstanceCreateInfo {
            s_type: VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
            p_next: ptr::null(),
            flags: 0,
            p_application_info: &app_info,
            enabled_layer_count: layers.len() as u32,
            pp_enabled_layer_names: if layer_ptrs.is_empty() { ptr::null() } else { layer_ptrs.as_ptr() },
            enabled_extension_count: extensions.len() as u32,
            pp_enabled_extension_names: if ext_ptrs.is_empty() { ptr::null() } else { ext_ptrs.as_ptr() },
        };

        let mut handle: VkInstance = ptr::null_mut();

        // Шаг 2: вызвать vkCreateInstance (глобальная функция).
        // ⚠ pAllocator = null → используется аллокатор Vulkan по умолчанию.
        // SAFETY: create_info полностью инициализирован, все указатели
        // указывают на живые данные, pAllocator = null (по умолчанию).
        let code = unsafe { (lib.create.inner)(&create_info, ptr::null(), &mut handle) };
        let res = VkResult::from(code);
        if !res.ok() {
            return Err(InstanceError::Vk(res));
        }
        let api_version = lib.api_version().unwrap_or(vk_make_api_version(0, 1, 0, 0));

        // Шаг 3: ТЕПЕРЬ можно запросить vkDestroyInstance — он привязан
        // к конкретному инстансу.
        let destroy: VkFunction<PfnDestroyInstance> = lib
            .instance_fn(handle, b"vkDestroyInstance\0")
            .ok_or(InstanceError::NoDestroyInstance)?;

        Ok(Instance { handle, destroy, api_version })
    }

    fn api_version(&self) -> u32 {
        self.api_version
    }

    fn handle(&self) -> VkInstance {
        self.handle
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // ⚠ Drop НЕ ДОЛЖЕН ПАНИКАТЬ. Здесь только вызов C-функции.
        if self.handle.is_null() {
            return;
        }
        // SAFETY: handle получен из vkCreateInstance и ещё не уничтожен;
        // pAllocator = null → освобождать нечего.
        unsafe { (self.destroy.inner)(self.handle, ptr::null()) };
    }
}

#[derive(Debug)]
enum InstanceError {
    Vk(VkResult),
    NoDestroyInstance,
}

impl std::fmt::Display for InstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstanceError::Vk(r) => write!(f, "vkCreateInstance: {r}"),
            InstanceError::NoDestroyInstance => {
                write!(f, "нет vkDestroyInstance для этого инстанса — загрузчик сломан")
            }
        }
    }
}

fn nul_bytes(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec().into_iter().chain(std::iter::once(0)).collect()
}

fn part_18_2() {
    harness::part("18.2", "ПРИМЕР: создание и уничтожение VkInstance");

    let Ok(lib) = VulkanLib::load("vulkan-1.dll") else {
        println!("  нет драйвера — пропуск");
        return;
    };

    // Валидатор только в debug. В release он слишком дорог.
    #[cfg(debug_assertions)]
    let want_validation = true;
    #[cfg(not(debug_assertions))]
    let want_validation = false;

    let layers: Vec<&str> = if want_validation {
        let available = lib.instance_layers().unwrap_or_default();
        if available.iter().any(|l| l == "VK_LAYER_KHRONOS_validation") {
            vec!["VK_LAYER_KHRONOS_validation"]
        } else {
            println!("  валидатор не найден, создаю без него");
            vec![]
        }
    } else {
        vec![]
    };

    match Instance::create(&lib, "novell-module18", &layers, &[]) {
        Ok(inst) => {
            println!("  VkInstance создан, apiVersion = 0x{:08x}", inst.api_version());
            println!("    слои: {layers:?}");
            // ⚠ Настоящий вулкан-слой теперь загружен. При Drop ниже
            //   он выгрузится.
            let _ = &inst;
            println!("  инстанс будет уничтожен при выходе из области (RAII)");
        }
        Err(e) => {
            println!("  {e}");
            println!("    ⚠ VK_ERROR_INCOMPATIBLE_DRIVER = нет ни одного совместимого драйвера");
        }
    }
    println!("  выход из области: Drop вызвал vkDestroyInstance");
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 18.3 — ТЕОРИЯ: RAII-обёртки и порядок уничтожения
// ─────────────────────────────────────────────────────────────────────────────
//
// ЭТО ГЛАВНЫЙ УРОК МОДУЛЯ. В Vulkan объекты образуют СТРУКТУРУ:
//
//   VkInstance
//     └── VkDevice
//           ├── VkBuffer        ─┐
//           ├── VkImage         │  должны умереть ДО VkDevice
//           ├── VkPipeline      │
//           ├── VkCommandPool  ─┘
//           └── VkCommandBuffer  (живут, пока жив command pool)
//
// В C++ ты пишешь деструкторы руками и надеешься, что не перепутал.
// Порядок — это соглашение, которое никто не проверяет. Ошибка даёт
// «объект уничтожен после устройства», а водитель может не заметить
// и упасть через 10 минут в другом месте.
//
// В Rust порядок ЗАДАЁТСЯ ПОЛЯМИ СТРУКТУРЫ — но правило НЕОЖИДАННОЕ:
//
//   ⚠ ЛОКАЛЬНЫЕ ПЕРЕМЕННЫЕ уничтожаются в ОБРАТНОМ порядке (LIFO),
//     а ПОЛЯ СТРУКТУРЫ — в ПРЯМОМ порядке объявления (FIFO)!
//
// Разница напечатана и проверена в модуле 01, часть 1.2. Значит для
// нашей структуры `device` объявляется ПОСЛЕДНИМ:
//   Gpu {
//       buffers: Vec<Buffer>,    // первое поле -> уничтожается ПЕРВЫМ
//       pipelines: Vec<Pipeline>,
//       device: Device,          // последнее поле -> уничтожается ПОСЛЕДНИМ ✓
//   }
//
// Ошибёшься в одну строку — получишь use-after-free в драйвере,
// который проявится через 10 минут в другом месте. Rust не поймает
// неправильный порядок полей, если ты о нём не подумал.
/// struct Buffer { /* VkBuffer + VkDeviceMemory */ }
/// struct Gpu { device: Device, buffers: Vec<Buffer> }
/// ```
///
/// Требования:
///   * Каждая обёртка: `new`, `handle()`, `Drop` (без паники!).
///   * `Buffer` внутри ДЕРЖИТ ссылку на устройство (`PhantomData<&Device>`
///     или `&'a Device` с lifetime) — компилятор не даст уничтожить
///     буфер раньше устройства, если буфер живёт дольше. ⚠ Объясни
///     в комментарии: почему одного порядка полей недостаточно и
///     зачем нужен lifetime.
///   * `Gpu` сначала объявляет `buffers`, потом `device` — и Drop
///     уничтожает буферы раньше устройства. Порядок ИМЕНно такой:
///     поля уничтожаются в порядке объявления, поэтому `device`
///     должен быть ПОСЛЕДНИМ полем.
///   * `unsafe impl Send для Device` — с обоснованием (Vulkan 1.x
///     позволяет использовать VkDevice из нескольких потоков).
///   * Проверь порядок: вставь println в каждый Drop и напечатай
///     последовательность при выходе из области.
///   * Создание объектов — имитация: без настоящего Device создать
///     Buffer нельзя, поэтому используй «магический» хэндл
///     (`0x1000 + n`) и `AtomicBool`-счётчик аллокаций.
fn task_18_2() {
    use std::marker::PhantomData;
    use std::sync::atomic::{AtomicU32, Ordering};

    static ALLOCATED: AtomicU32 = AtomicU32::new(0);
    static FREED: AtomicU32 = AtomicU32::new(0);

    struct Device {
        handle: u64,
    }

    impl Device {
        fn new() -> Self {
            ALLOCATED.fetch_add(1, Ordering::Relaxed);
            Device { handle: 0x1000 }
        }
        fn handle(&self) -> u64 {
            self.handle
        }
    }

    impl Drop for Device {
        fn drop(&mut self) {
            FREED.fetch_add(1, Ordering::Relaxed);
            println!("    [Drop] VkDevice 0x{:x} — ПОСЛЕДНИМ", self.handle);
        }
    }

    /// Lifetime `'d` привязывает буфер к устройству: пока жив буфер,
    /// живо и устройство. Это СТАТИЧЕСКАЯ гарантия, которой нет
    /// в C++. Одного порядка полей мало — он защищает только в
    /// пределах одной структуры, а буфер может лежать в другом
    /// месте (Vec<Gpu>), и тогда нужна связь через тип.
    struct Buffer<'d> {
        handle: u64,
        size: usize,
        _device: PhantomData<&'d Device>,
    }

    impl<'d> Buffer<'d> {
        fn new(device: &'d Device, size: usize) -> Self {
            assert!(size > 0, "размер буфера должен быть задан");
            ALLOCATED.fetch_add(1, Ordering::Relaxed);
            Buffer { handle: 0x2000 + size as u64, size, _device: PhantomData }
        }
        fn handle(&self) -> u64 {
            self.handle
        }
        fn size(&self) -> usize {
            self.size
        }
    }

    impl Drop for Buffer<'_> {
        fn drop(&mut self) {
            FREED.fetch_add(1, Ordering::Relaxed);
            println!("    [Drop] VkBuffer 0x{:x} ({} B) — ДО устройства", self.handle, self.size);
        }
    }

    // SAFETY: VkDevice в Vulkan 1.x разрешено использовать из нескольких
    // потоков при外部 синхронизации доступа к его полям (очередям).
    unsafe impl Send for Device {}

    struct Gpu<'d> {
        // ⚠ device ПОСЛЕДНИМ: поля уничтожаются в ПРЯМОМ порядке
        //   объявления, поэтому последнее поле умрёт последним.
        buffers: Vec<Buffer<'d>>,
        device: Device,
    }

    println!("  создаём Gpu...");
    {
        let dev = Device::new();
        let gpu = Gpu { device: dev, buffers: Vec::new() };
        // ⚠ Здесь нужен доступ к device, но поле принадлежит gpu.
        //   Поэтому в реальном коде делают либо Arc<Device>, либо
        //   разделяют владение через Rc/Arc. Здесь — создаём буферы
        //   ДО того, как структура собрана:
        println!("  выход из области: смотри порядок Drop");
        drop(gpu);
    }
    println!("  allocated={} freed={} (должны совпасть)",
        ALLOCATED.load(Ordering::Relaxed), FREED.load(Ordering::Relaxed));
    assert_eq!(ALLOCATED.load(Ordering::Relaxed), FREED.load(Ordering::Relaxed));
}

/// ЧАСТЬ 18.6 — ЗАДАНИЕ 18.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Builder-паттерн для структур Vulkan. Ровно то, что делает
/// `ash::vk::InstanceCreateInfo::builder()`.
///
/// ```ignore
/// struct InstanceCreateInfo<'a> { /* ... repr(C) ... */ }
///
/// struct InstanceCreateInfoBuilder<'a> { /* промежуточное состояние */ }
///
/// impl<'a> InstanceCreateInfoBuilder<'a> {
///     fn new() -> Self;
///     fn app_name(self, &'a str) -> Self;      // сохраняет ССЫЛКУ
///     fn api_version(self, u32) -> Self;
///     fn layers(self, &'a [&'a str]) -> Self;
///     fn extensions(self, &'a [&'a str]) -> Self;
///     fn build(self) -> Result<InstanceCreateInfo<'a>, BuilderError>;
/// }
///
/// struct ToCStrings { bufs: Vec<Vec<u8>>, ptrs: Vec<*const u8> }
/// impl ToCStrings {
///     fn new(items: &[&str]) -> Self;          // NUL-терминаторы
///     fn as_ptr(&self) -> *const *const u8;    // null, если пусто
/// }
/// ```
///
/// ТРЕБОВАНИЯ, КОТОРЫЕ ДЕЛАЮТ ЭТО НЕ «ПРОСТО»:
///
///   1. Словари живут внутри builder'а. И они должны жить ДО вызова
///      `vkCreateInstance`, и ПОСЛЕ сборки структуры. Это и есть
///      смысл lifetime `'a` в сигнатуре. Без него «словари умерли
///      досрочно» — классический use-after-free, и в C++ он такой же.
///   2. `build()` возвращает `Result`, а не структуру: без app_name
///      или api_version структура невалидна по спецификации.
///   3. `as_ptr` возвращает `null` для пустого списка — Vulkan
///      различает «нет расширений» (nullptr) и «пустой массив».
///      ⚠ Наивная реализация вернёт указатель на пустой Vec, и
///      драйвер может это отвергнуть.
///
/// Проверь:
///   * `build()` без app_name → `Err(BuilderError::NoAppName)`.
///   * `build()` без api_version → `Err(BuilderError::NoApiVersion)`.
///   * Пустые слои/расширения дают `null`, а не указатель на пустое.
///   * `s_type` проставляется автоматически и равен 1.
fn task_18_3() {
    const S_TYPE_INSTANCE_CREATE_INFO: i32 = 1;

    #[derive(Debug, PartialEq)]
    enum BuilderError {
        NoAppName,
        NoApiVersion,
    }

    /// Преобразует список &str в массив NUL-терминированных C-строк.
    /// ⚠ Хранит И буферы, И указатели. Если хранить только указатели,
    ///   они укажут в никуда: временные Vec из `to_vec()` уже умерли.
    #[derive(Debug)]
    struct ToCStrings {
        bufs: Vec<Vec<u8>>,
        ptrs: Vec<*const u8>,
    }

    impl ToCStrings {
        fn new(items: &[&str]) -> Self {
            not_yet!("bufs: Vec<u8> с NUL, ptrs: указатели на bufs");
        }

        fn as_ptr(&self) -> *const *const u8 {
            not_yet!("if ptrs.is_empty() -> std::ptr::null(), иначе ptrs.as_ptr()");
        }

        fn len(&self) -> usize {
            self.ptrs.len()
        }
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct AppInfo {
        s_type: i32,
        p_next: *const u8,
        p_app_name: *const u8,
        app_version: u32,
        p_engine_name: *const u8,
        engine_version: u32,
        api_version: u32,
    }

    #[repr(C)]
    struct InstanceCreateInfo {
        s_type: i32,
        p_next: *const u8,
        flags: u32,
        p_app_info: *const AppInfo,
        layer_count: u32,
        pp_layers: *const *const u8,
        ext_count: u32,
        pp_extensions: *const *const u8,
    }

    struct Builder<'a> {
        app_name: Option<&'a str>,
        app_version: u32,
        api_version: Option<u32>,
        layers: Option<&'a [&'a str]>,
        extensions: Option<&'a [&'a str]>,
    }

    impl<'a> Builder<'a> {
        fn new() -> Self {
            not_yet!("всё None, api_version None");
        }
        fn app_name(mut self, n: &'a str) -> Self {
            not_yet!("self.app_name = Some(n)");
            self
        }
        fn app_version(mut self, v: u32) -> Self {
            not_yet!("self.app_version = v");
            self
        }
        fn api_version(mut self, v: u32) -> Self {
            not_yet!("self.api_version = Some(v)");
            self
        }
        fn layers(mut self, l: &'a [&'a str]) -> Self {
            not_yet!("self.layers = Some(l)");
            self
        }
        fn extensions(mut self, e: &'a [&'a str]) -> Self {
            not_yet!("self.extensions = Some(e)");
            self
        }

        /// ⚠ СБОРКА ТРЕБУЕТ ВЛАДЕНИЯ: возвращаем и структуру, и
        ///   C-строки, потому что структура указывает на них.
        ///   Если вернуть только структуру — указатели укажут на
        ///   локальные переменные, которые умрут сразу после return.
        fn build(self) -> Result<(InstanceCreateInfo, ToCStrings, ToCStrings, AppInfo), BuilderError> {
            not_yet!("проверь app_name и api_version, собери всё");
        }
    }

    // --- Проверки ToCStrings ------------------------------------------------
    let empty = ToCStrings::new(&[]);
    assert!(empty.as_ptr().is_null(), "пустой список → null, не указатель на пустое");
    assert_eq!(empty.len(), 0);

    let one = ToCStrings::new(&["VK_KHR_surface"]);
    assert!(!one.as_ptr().is_null());
    assert_eq!(one.len(), 1);
    // ⚠ SAFETY: буферы живут, пока живёт `one`; Vulkan читает строки
    //   только во время вызова vkCreateInstance.
    assert_eq!(unsafe { std::ffi::CStr::from_ptr(*one.as_ptr() as *const std::ffi::c_char) }.to_str().unwrap(), "VK_KHR_surface");

    // --- Проверки builder'а -------------------------------------------------
    let r = Builder::new().app_name("x").api_version(1).build();
    assert!(r.is_ok());

    match Builder::new().api_version(1).build() {
        Err(BuilderError::NoAppName) => {}
        Ok(_) => panic!("ожидалась ошибка NoAppName, а build() прошёл"),
        Err(e) => panic!("ожидалась NoAppName, получено {e:?}"),
    }
    match Builder::new().app_name("x").build() {
        Err(BuilderError::NoApiVersion) => {}
        Ok(_) => panic!("ожидалась ошибка NoApiVersion, а build() прошёл"),
        Err(e) => panic!("ожидалась NoApiVersion, получено {e:?}"),
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 18.3 — ПРИМЕР: RAII-обёртки и порядок уничтожения
// ─────────────────────────────────────────────────────────────────────────────
use std::mem;

fn part_18_3() {
    harness::part("18.3", "ПРИМЕР: порядок drop в структуре-объётке");

    // ⚠ КЛЮЧЕВОЙ ФАКТ, КОТОРЫЙ ЛОМАЕТ СИЛУ:
    //    ЛОКАЛЬНЫЕ переменные  → обратный порядок объявления (LIFO)
    //    ПОЛЯ СТРУКТУРЫ        → ПРЯМОЙ  порядок объявления (FIFO)
    // Разница напечатана в модуле 01, часть 1.2.

    struct Named(&'static str);
    impl Drop for Named {
        fn drop(&mut self) {
            println!("    drop {}", self.0);
        }
    }

    struct Gpu {
        buffer: Named,
        pipeline: Named,
        // ⚠ device ПОСЛЕДНИМ: поля уничтожаются в порядке объявления,
        //   значит последнее поле умрёт последним.
        device: Named,
    }

    println!("  структура Gpu (поля: buffer, pipeline, device):");
    {
        let _g = Gpu {
            buffer: Named("VkBuffer  (должен умереть ПЕРВЫМ)"),
            pipeline: Named("VkPipeline"),
            device: Named("VkDevice  (должен умереть ПОСЛЕДНИМ)"),
        };
        println!("    ...выходим из области");
    }
    println!("  ✓ порядок правильный: буфер и пайплайн до устройства");

    println!("\n  а теперь переставим поля «для красоты» (device первым):");
    struct GpuWrong {
        device: Named,
        buffer: Named,
    }
    {
        let _g = GpuWrong {
            device: Named("VkDevice  (умер ПЕРВЫМ — это баг)"),
            buffer: Named("VkBuffer   (умер ПОСЛЕДНИМ — use-after-free)"),
        };
    }
    println!("  ✗ вот так получается «объект уничтожен после устройства».");
    println!("    Компилятор молчит. Валидатор Vulkan скажет. А без него —");
    println!("    драйвер упадёт через 10 минут в другом месте.");

    // --- Типичная утечка: «отдал владение C-коду» --------------------------
    println!("\n  --- mem::forget как «я знаю, что делаю» ---");
    struct LeakyHandle {
        handle: u64,
        _owned: Option<Box<u8>>,
    }
    let h = LeakyHandle { handle: 0xABCD, _owned: Some(Box::new(1)) };
    let raw = h.handle;
    mem::forget(h);
    println!("  mem::forget: handle 0x{raw:x} живёт вечно, Box никогда не освободится");
    println!("    ⚠ В C++ то же самое — «забыл delete», и ищется вручную.");
    println!("      Здесь хотя бы СЛОВО forget говорит: «я знаю, что делаю».");
    let _ = raw;

    // --- Чек-лист для настоящего движка ------------------------------------
    println!("\n  Что дальше в настоящем движке (порядок вызовов):");
    println!("    1. Surface — нужен реальный десктоп. Пока окна нет, Vulkan");
    println!("       не запустится. Для тестов: VK_KHR_headless_surface.");
    println!("    2. Instance + Debug Messenger (EXT_debug_utils) — ошибки");
    println!("       приходят в Rust, а не в stderr.");
    println!("    3. Перечисление PhysicalDevice: выбрать по deviceType");
    println!("       и лимитам (maxUniformBufferRange, maxPushConstantsSize).");
    println!("    4. Device + очереди. Очередь — часть Device, но живёт");
    println!("       своей жизнью и уничтожается отдельно.");
    println!("    5. Swapchain: зависит от Surface, а НЕ от Device. Пересоздаётся");
    println!("       при resize — и это главный источник «утечек» в движках.");
    println!("    6. CommandPool → CommandBuffer → Fence. Один пул на поток.");
    println!("    7. Pipeline + DescriptorSetLayout + PipelineLayout + RenderPass.");
    println!("    8. Всё это — поля одной структуры App, и Drop разрулит");
    println!("       порядок. Именно ради этого весь модуль и написан.");
}

/// ЧАСТЬ 18.4 — ЗАДАНИЕ 18.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Довести загрузчик до продакшн-вида: перечисление устройств и
/// разбор версии.
///
/// Требования:
///   * Используй `VulkanLib` и `Instance` из примера — не пиши заново.
///   * `version_string(v: u32) -> String` — разобрать API-версию.
///   * `physical_devices(&self, instance) -> Result<Vec<(String, u32)>, VkResult>`
///     — имя устройства и его `apiVersion`.
///   * ⚠ Структуры `VkPhysicalDeviceProperties` объявляем ТОЛЬКО
///     префиксом: нужные поля идут подряд с начала, поэтому смещения
///     совпадают с C. Объясни в комментарии, почему префикс БЕЗОПАСЕН,
///     а пропуск поля в середине — нет.
///   * Двухпроходная схема подсчёта (count, затем заполнение).
///   * Обработка `VK_INCOMPLETE`: повторить с увеличенным буфером.
///   * Никаких `unwrap`.
///
/// ⚠ ВНИМАНИЕ: функции берутся ПО ИНСТАНСУ, поэтому передавай
///   созданный `Instance`, а не `NULL`.
fn task_18_1() {
    const VK_MAX_PHYSICAL_DEVICE_NAME_SIZE: usize = 256;

    /// ⚠ ПРЕФИКС `VkPhysicalDeviceProperties`.
    ///   Все нужные поля идут подряд с начала структуры, поэтому их
    ///   смещения совпадают с C-версией. Если бы мы пропустили поле
    ///   В СЕРЕДИНЕ (например `deviceName`), все последующие
    ///   смещения съехали бы, и драйвер записал бы данные не туда.
    ///   Именно поэтому в настоящем `ash` структура объявлена целиком.
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct VkPhysicalDeviceProperties {
        api_version: u32,
        driver_version: u32,
        vendor_id: u32,
        device_id: u32,
        device_type: i32,
        device_name: [u8; VK_MAX_PHYSICAL_DEVICE_NAME_SIZE],
    }

    type PfnEnumeratePhysicalDevices =
        unsafe extern "system" fn(*mut c_void, *mut u32, *mut *mut c_void) -> i32;
    type PfnGetPhysicalDeviceProperties =
        unsafe extern "system" fn(*mut c_void, *mut VkPhysicalDeviceProperties);

    fn version_string(v: u32) -> String {
        not_yet!("variant = v>>29, major = (v>>22)&0x7F, minor = (v>>12)&0x3FF, patch = v&0xFFF");
    }

    fn physical_devices(
        lib: &VulkanLib,
        instance: VkInstance,
    ) -> Result<Vec<(String, u32)>, VkResult> {
        not_yet!("двухпроходная схема + GetPhysicalDeviceProperties для каждого");
    }

    fn supports(lib: &VulkanLib, ext: &str) -> bool {
        not_yet!("сравни со списком instance_extensions");
    }

    let Ok(lib) = VulkanLib::load("vulkan-1.dll") else {
        panic!("нет драйвера: задание 18.1 невыполнимо. Установи Vulkan Runtime.");
    };

    assert_eq!(version_string(vk_make_api_version(0, 1, 3, 275)), "1.3.275");
    assert!(supports(&lib, "VK_KHR_surface"));

    let Ok(inst) = Instance::create(&lib, "novell-module18", &[], &[]) else {
        panic!("не удалось создать VkInstance");
    };
    let devices = physical_devices(&lib, inst.handle()).expect("перечисление устройств");
    println!("  устройств: {}", devices.len());
    for (name, ver) in &devices {
        println!("    - {name} (api {})", version_string(*ver));
    }
    assert!(!devices.is_empty(), "хотя бы одно устройство должно быть");
}
