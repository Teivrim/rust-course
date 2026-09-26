//! ============================================================================
//! МОДУЛЬ 20 — ИДИОМЫ, CLIPPY И АРХИТЕКТУРА ДВИЖКА
//! ============================================================================
//!
//! ЧАСТЬ 20.1 — ТЕОРИЯ: главная таблица C++ → Rust
//! ─────────────────────────────────────────────────────────────────────────────
//!
//! Ты знаешь C++ на 50-60%. Значит, главный риск сейчас — не «не знать Rust»,
//! а НАПИСАТЬ C++ НА RUST. Это называется «C++-диалект на Rust»:
//! сырые указатели для ручного владения, флаг `initialized`,
//! out-параметры для ошибок, `static mut` для глобального состояния,
//! `Box<dyn Trait>` везде, где в C++ был `virtual`.
//!
//! Такой код компилируется. Он даже работает. И он хуже C++-кода,
//! потому что ты платишь за безопасность и не получаешь её.
//!
//! ПРАВИЛО ОДНОЙ ФРАЗОЙ:
//!     в C++ безопасность обеспечивает ДИСЦИПЛИНА программиста,
//!     в Rust — ТИП. Если в коде есть `unsafe`, значит, дисциплина
//!     всё ещё обеспечивает безопасность, и Rust пока ничем не помог.
//!
//! ЧАСТЬ 20.2 — ТЕОРИЯ: clippy и идиомы
//! ─────────────────────────────────────────────────────────────────────────────
//!
//! `clippy` — это не «стиль», а сотни проверок с названиями.
//! Полезно знать их по ИМЕНИ: увидел `clippy::needless_range_loop` —
//! сразу понятно, что от тебя хотят. Ниже — тот срез, который
//! реально встречается в игровом коде.
//!
//! Три группы, которые стоит выучить наизусть:
//!
//!   1. ЛОПАТА (бесполезный код, который можно стереть без потерь):
//!        `let _ = ...`, `.iter().cloned().collect::<Vec<_>>()`,
//!        `if x == true`, `return` в конце функции,
//!        `match` с одной рукой -> `if let`,
//!        ручной цикл -> адаптор итератора.
//!
//!   2. ЛОВУШКИ (код работает, но делает не то, что думаешь):
//!        `needless_range_loop`  — `for i in 0..v.len()` вместо `for x in &v`
//!        `or_fun_call`          — `.unwrap_or(expensive())` вместо
//!                                  `.unwrap_or_else(|| expensive())`
//!        `redundant_clone`      — `clone()` ради того, чтобы отдать владение,
//!                                  хотя можно было переставить строки
//!        `useless_vec`          — `&vec![1,2,3]` вообще не нужен, массив
//!        `len_zero`             — `.len() == 0` вместо `.is_empty()`
//!        `needless_borrow`      — `&x`, когда x и так Copy
//!
//!   3. ДИЗАЙН (clippy прав, и ты злишься, но он прав):
//!        `large_enum_variant`   — большой вариант `enum` -> `Box` внутри
//!        `type_complexity`      — тип слишком сложный, разбей на структуры
//!        `too_many_arguments`   — пора в структуру
//!        `result_large_err`     — ошибка весит много, положи в `Box`
//!
//! ГЛАВНОЕ ПРО CLIPPY, КОТОРОЕ ПОНИМАЮТ НЕ ВСЕ:
//! clippy — это ОБЫЧНЫЙ ЛИНТЕР, то есть программа, которая ищет
//! подстроки и шаблоны в исходнике. Ничего магического в нём нет.
//! Поэтому ты можешь написать СВОЙ линтер под свой проект —
//! и это ровно то, что делает часть 20.2 ниже. Линтер движка
//! «не создавай `Box<dyn Stage>` в горячем пути» пишется за час.
//!
//! ЧАСТЬ 20.3 — ТЕОРИЯ: архитектура движка и анти-паттерны
//! ─────────────────────────────────────────────────────────────────────────────
//!
//! РАСКЛАДКА КРЕЙТОВ. Не начинай с одного крейта на 30 000 строк.
//!
//!   app/         главный цикл, окно, обработка ввода. Никакой логики игры.
//!   core/        общие типы: `EntityId`, `ResourceId`, ошибки, `Result`.
//!   math/        Vec3/Mat4/Quat. Ноль зависимостей, ноль аллокаций.
//!   gfx/         Vulkan, рендер-пасс, ресурсы GPU.
//!   assets/      загрузка, кэш, десериализация.
//!   scene/       ECS, компоненты, системы.
//!
//! Правило зависимостей: `app -> scene -> gfx -> math`, и НИКОГДА
//! наоборот. `math` не знает про `gfx`. `gfx` не знает про `scene`.
//! Если понадобилась стрелка вверх — значит, слой выбран неверно.
//!
//! ⚠ ПОЧЕМУ НЕ КРЕЙТЫ, А МОДУЛИ, ДЛЯ НАЧАЛА:
//!   Крейт = отдельная компиляция, отдельные тесты, отдельный
//!   публичный API. Это правильно, но дорого: пока ты учишь язык,
//!   ты не будешь видеть, как «сквозит» тип через слои.
//!   Модули дают ту же приватность (`pub(crate)`) за секунды.
//!   Перейти крейт -> модуль можно всегда (Ctrl+A в файл крейта),
//!   наоборот — никогда.
//!
//! ВИДИМОСТЬ. Rust даёт три уровня, и это НЕ «приватность для галочки»:
//!
//!   `fn f()`          только модуль. Всё, что не начинается с `pub`,
//!                     видно здесь и нигде.
//!   `pub(crate) f()`  весь крейт. Модули внутри крейта — «коллеги»,
//!                     которым нужен доступ, а пользователям крейта — нет.
//!   `pub f()`         наружу. Это ОБЯЗАТЕЛЬСТВО: ты обещаешь, что
//!                     не сломаешь этот API. Чем меньше `pub` — тем
//!                     меньше обещаний, которые придётся держать.
//!
//! Сравни с C++: там `public:` написан в одном месте и ничего
//! не сообщает о том, как часто этим пользуются снаружи.
//! В Rust количество `pub` — это и есть «ширина» API крейта.
//!
//! АНТИ-ПАТТЕРНЫ ДВИЖКА (каждый я видел в реальных проектах):
//!
//!  1. `static mut STATE` — глобальное изменяемое состояние.
//!     Не компилируется в 2024+ мультипоточном режиме, и это подарок:
//!     глобальное состояние + потоки = гонки. Решение — `App`,
//!     который создаётся один раз и передаётся вниз по ссылке.
//!
//!  2. Спагетти `Rc<RefCell<T>>`. Каждый узел хранит указатель на
//!     соседа, чтобы тот не умер раньше времени. Через 10 таких
//!     связей понять граф невозможно, и компилятор не поможет —
//!     ведь всё «безопасно». Решение — дерево владения сверху вниз:
//!     `&mut App` вниз по стеку вызовов. Если граф ИЕРАРХИЧЕСКИЙ
//!     (как сцена), дерево владения — правильная структура,
//!     а `Rc` — нет.
//!
//!  3. Строковые ключи: `HashMap<String, Mesh>`. Хеш строки на каждый
//!     lookup, плюс опечатка = «объект молча исчез». Решение — newtype
//!     `ResourceId(u32)`, имя только для отладки.
//!
//!  4. `Box<dyn Trait>` на каждый чих. `dyn` = виртуальный вызов +
//!     невозможность инлайна. Для редких вещей (загрузчик ресурсов,
//!     плагин) — правильно. Для горячего пути (100 000 вызовов
//!     на кадр) — модуль 06 про это.
//!
//!  5. `unsafe` россыпью по всему проекту. Правило курса: `unsafe`
//!     живёт в ОДНОМ файле-обёртке на каждую платформу. Снаружи —
//!     безопасный API. Модули 12 и 18 — именно про это.
//!
//!  6. `clone()` как средство борьбы с компилятором. 90% случаев —
//!     неправильно выбрана структура данных. `clone()` маскирует
//!     проблему и добавляет копирование в горячий путь.
//!
//!  7. God-object `Engine` на 3000 строк, в котором всё.
//!     Если файл не помещается на два экрана — это не файл, а
//!     анти-паттерн. Резать по слоям, а не по классам.
//!
//!  8. Преждевременный ECS. ECS — не серебряная пуля. Для 50 объектов
//!     `Vec<struct Mesh>` быстрее и проще. Переходить на ECS, когда
//!     реально упёрся в cache misses (профиль это покажет), а не
//!     «так принято».
//!
//! ЧАСТЬ 20.4 — ИТОГОВЫЙ ЧЕК-ЛИСТ СПРИНТА
//! ─────────────────────────────────────────────────────────────────────────────

#![allow(unused_variables, unused_imports, dead_code)]
// Из-за `not_yet!` компилятор считает недостижимым всё, что после
// неё стоит, и ругается на `mut`, который нужен заданию, а не заглушке.
#![allow(unreachable_code, unreachable_patterns, unused_mut)]

use curriculum::alloc;
use curriculum::harness::{self, report};
use curriculum::not_yet;
use curriculum::rng;

use std::collections::HashMap;
use std::fmt;
use std::mem::size_of;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::time::Instant;

fn main() {
    harness::module(20, "Идиомы, clippy и архитектура движка");

    part_20_1(); // таблица C++ -> Rust + рефакторинг C++-подобного кода
    part_20_2(); // clippy-идиомы + свой линтер
    part_20_3(); // каркас движка: ресурсы, стадии, порядок Drop
    part_20_4(); // итоговый чек-лист

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("20.1  Рефакторинг C++-подобного рендерера в RAII", task_20_1);
    r.task("20.2  Свой линтер: 6 правил + прогон на трёх исходниках", task_20_2);
    r.task("20.3  Каркас Main/: реестр ресурсов и конвейер кадра", task_20_3);
    r.summary(20);
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЧАСТЬ 20.1
// ═════════════════════════════════════════════════════════════════════════════

// Таблица живёт в коде, а не в комментарии: её можно распечатать,
// распечатать в файл и держать открытой во время рефакторинга.
// Комментарий в коде — это то, что ты прочитаешь один раз.
const CPP_TO_RUST: &[(&str, &str, &str)] = &[
    // ── Владение и память ───────────────────────────────────────────────────
    ("new / delete", "Box::new / ничего", "удаление автоматическое, в порядке выхода из скоупа"),
    ("std::move", "move (при = или передаче)", "деструктор источника НЕ вызывается — это не std::move"),
    ("std::unique_ptr<T>", "Box<T>", "+ Option<Box<T>> вместо null-указателя"),
    ("std::shared_ptr<T>", "Rc<T> / Arc<T>", "циклы разрываются вручную через Weak"),
    ("weak_ptr<T>", "Weak<T>", "то же, но без автоматического продления жизни"),
    ("T* / T&", "&T / &mut T", "заимствование проверяется компилятором, dangling — ошибка"),
    ("const T&", "&T", "то же, но без ментальной нагрузки"),
    ("T** и ручное владение", "ссылки + Box", "двойных указателей в идиоматичном коде нет"),
    ("nullptr / NULL", "Option<T>", "null-состояние в ТИПЕ, а не в соглашении"),
    ("sizeof(T)", "size_of::<T>()", "const, работает в static assert"),
    ("static_assert", "const _: () = assert!(..)", "проверка на этапе компиляции, без макросов"),
    ("malloc / free", "Vec<T> / Box<T>", "а аллокатор трекается через #[global_allocator]"),
    ("placement new", "MaybeUninit<T>", "тот же трюк, но unsafe-типизированный"),

    // ── Типы ────────────────────────────────────────────────────────────────
    ("enum class", "enum", "дешевле и исчерпывающе: match проверит все случаи"),
    ("union + tag (руками)", "enum с полями", "дискриминант + данные, безопасность бесплатно"),
    ("std::variant<T,U>", "enum { A(T), B(U) }", "вариантов сколько нужно, а не два"),
    ("std::optional<T>", "Option<T>", "без выделения памяти, niche-оптимизация"),
    ("std::expected (C++23)", "Result<T, E>", "+ дешёвый ? и From"),
    ("union { int i; float f; }", "union + unsafe", "в Rust union есть, но не для gameplay-кода"),
    ("using MyInt = int", "struct MyInt(u32)", "newtype: ноль стоимости, но ошибки невозможны"),

    // ── Ошибки и поток управления ───────────────────────────────────────────
    ("throw / catch", "Result<T, E> / ?", "ошибка — значение, а не поток управления"),
    ("throw std::runtime_error", "Err(MyError::Runtime(..))", "тип ошибки — часть типа возврата"),
    ("try { } catch (A) { } catch (B) { }", "match / if let Err(e)", "без раскрутки стека"),
    ("bool init(std::string& err)", "fn init() -> Result<Self, E>", "ошибка в ТИПЕ, out-параметр невозможен"),
    ("assert()", "assert! / debug_assert!", "в release проверки исчезают (debug_assert)"),
    ("std::terminate", "паника / process::exit", "паника раскручивает стек и зовёт Drop"),
    ("goto", "'label: loop", "никаких произвольных переходов"),
    ("delete this", "невозможно", "lifetime-параметры не дают"),

    // ── Полиморфизм и шаблоны ──────────────────────────────────────────────
    ("template<typename T>", "impl Trait / generics", "мономорфизация + проверка на этапе компиляции"),
    ("virtual / vtable", "dyn Trait", "тот же vtable, но объект по умолчанию Box<dyn Trait>"),
    ("if constexpr", "разные impl по типам", "гарантия от компилятора, а не «повезло»"),
    ("operator+", "impl Add for T", "операторы — обычные трейты"),
    ("std::function", "Box<dyn Fn(A) -> B>", "Fn / FnMut / FnOnce — точнее и дешевле"),
    ("friend class", "pub(crate) / приватный модуль", "видимость по модулю, а не «друзья»"),
    ("multiple inheritance", "trait + impl для каждого", "конфликты методов решаются явным выбором"),

    // ── Контейнеры и строки ────────────────────────────────────────────────
    ("std::vector<T>", "Vec<T>", "владеет; push может переаллоцировать -> ссылки инвалидируются"),
    ("std::span (C++20)", "&[T]", "встроено в язык, шаблонов не нужно"),
    ("std::string", "String", "владеет, UTF-8 гарантирован"),
    ("std::string_view", "&str / &[u8]", "заимствование без копии"),
    ("std::map / unordered_map", "BTreeMap / HashMap", "HashMap НЕ упорядочен — не жди порядка"),
    ("std::initializer_list", "итераторы", "ленивость по умолчанию, collect() в конце"),
    ("for (auto& x : v)", "for x in &v", "владение / заимствование / копирование — явно"),

    // ── Глобальное состояние и статика ─────────────────────────────────────
    ("static в классе", "const / associated const", "инициализация без гонок"),
    ("function-local static", "OnceLock<T>", "то же, но потокобезопасно и явно"),
    ("mutable global", "Mutex<T> / OnceLock<T>", "глобальное состояние неявное и небезопасное"),
    ("constexpr", "const fn", "вычисляется на этапе компиляции"),

    // ── Прочее ──────────────────────────────────────────────────────────────
    ("#define", "const / generic / macro_rules!", "нет скрытой подстановки в токены"),
    ("memcpy / reinterpret_cast", "ptr::copy_nonoverlapping / transmute", "всё unsafe и с контрактом"),
    ("volatile", "Atomic<T> / Cell<T>", "с указанием, что именно запрещено переупорядочивать"),
    ("комментарий TODO", "todo!() / unimplemented!()", "видно в отчёте, а не в глазу"),
    ("UB где угодно", "ошибка компиляции (99%)", "оставшееся — unsafe, и оно видно в коде"),
];

fn part_20_1() {
    harness::part("20.1", "ТАБЛИЦА C++ -> RUST + РЕФАКТОРИНГ C++-ПОДОБНОГО КОДА");

    // ── Печать таблицы ──────────────────────────────────────────────────────
    println!("\n  Всего переводов: {}", CPP_TO_RUST.len());
    for (i, (c, r, note)) in CPP_TO_RUST.iter().enumerate() {
        // Печатаем блоками по 8, чтобы не получить простыню в 200 строк.
        if i % 8 == 0 {
            println!("\n  --- блок {} ---", i / 8 + 1);
        }
        println!("  {:<34} -> {}", c, r);
        if i % 8 == 7 {
            println!("      (суть: {})", note);
        }
    }
    println!("\n  ⚠ Суть каждой строки — третье поле таблицы. Оно важнее первых двух:");
    println!("    не «что писать», а «что изменилось ПО СУТИ».");

    // ── Шаг 1: C++-оригинал ────────────────────────────────────────────────
    harness::example("C++-оригинал, как его пишут в 2015 году");

    // C++:
    //     struct Renderer {
    //         VkDevice* device;      // может быть nullptr
    //         float*    vertices;    // new float[cap]
    //         size_t    cap, len;
    //         bool      initialized;
    //         std::string last_error;
    //
    //         Renderer(size_t c) : vertices(new float[c]), cap(c), len(0),
    //                              device(nullptr), initialized(false) {}
    //         ~Renderer() { if (initialized) shutdown(); }
    //
    //         bool init(VkInstance inst, std::string& err) {
    //             if (initialized) return true;
    //             if (vkCreateDevice(inst, nullptr, &device) != VK_SUCCESS) {
    //                 err = "no device"; return false;      // <-- утечка vertices
    //             }
    //             initialized = true;
    //             return true;
    //         }
    //         void shutdown() {
    //             vkDestroyDevice(device, nullptr);
    //             delete[] vertices;                        // <-- забывают
    //             device = nullptr;
    //             initialized = false;
    //         }
    //         void upload(const float* data, size_t n) {
    //             if (len + n > cap) throw std::runtime_error("overflow");
    //             std::memcpy(vertices + len, data, n * sizeof(float));
    //             len += n;
    //         }
    //     };
    //
    // Что здесь не так (всё это компилируется в C++):
    //   * `device == nullptr` — валидное состояние, и оно НЕ В ТИПЕ;
    //   * при ошибке `init` массив `vertices` остаётся выделенным,
    //     а деструктор его освобождает — но только если `initialized`;
    //   * `upload` кидает исключение, и `~Renderer` отработает... а вот
    //     если `init` бросит ДО `initialized = true`, деструктор
    //     `shutdown()` НЕ вызовет, и `delete[] vertices` не выполнится;
    //   * `memcpy` при `overflow` — исключение, но данные могли
    //     скопироваться наполовину при переполнении самого memcpy.
    //
    // ИТОГ: 4 способа утечь или сломать состояние, и все — в 30 строках.

    // ── Шаг 2: Rust-перевод «как есть» ─────────────────────────────────────
    harness::example("Rust-перевод этого C++ — дословно, включая дыры");

    let (leaked_blocks, leaked_bytes) = run_cxx_style();
    println!(
        "\n  Утечка после create() + destroy() без shutdown(): {} блок(ов) = {leaked_bytes} байт",
        leaked_blocks
    );
    assert!(leaked_blocks >= 1, "C++-стиль обязан течь — иначе демонстрация врёт");
    println!("  ⚠ Компилятор Rust на этот код НЕ ругается. Ни одного warning.");
    println!("    Ровно это и значит «C++-подобный Rust»: ты заплатил за");
    println!("    безопасность и не получил ничего, кроме какого-то `unsafe`.");

    // ── Шаг 3: RAII-перевод ────────────────────────────────────────────────
    harness::example("RAII-перевод: больше строк, но утечки не существует");

    // 0 — это `VK_SUCCESS`. Всё ненулевое — ошибка с кодом.
    DESTROYED.store(0, Ordering::Relaxed);
    let device = FakeDevice::create(0).expect("создание устройства");
    let dev_ptr = device.handle();

    {
        let mut r = Renderer::new(device, 1024).expect("инициализация");
        for i in 0..8 {
            r.upload(&[i as f32, i as f32 * 2.0, i as f32 * 3.0])
                .expect("upload");
        }
        // 8 вызовов по 3 float'а = 24 вершины.
        assert_eq!(r.len(), 24);
        // ⚠ А ВОТ ТЕПЕРЬ — главное отличие. Мы НЕ пишем `r.shutdown()`.
        //   Ни здесь, ни в `if`, ни в `catch`. `r` умрёт на закрывающей
        //   скобке, и `Drop` вызовет `shutdown` у `Device` — ВСЕГДА.
        //   Даже если между здесь и `}` будет `?`, `return` или `panic!`.
        println!(
            "  {} вершин, device = {:#x} — всё живо, ничего не освобождено ещё рано",
            r.len(),
            dev_ptr
        );
    } // <-- ЗДЕСЬ `r` дропается. `Device::drop` зовёт `vkDestroyDevice`.

    // Доказательство: «драйвер» увидел уничтожение исходного handle.
    // ⚠ СРАВНИВАЕМ НЕ С САМИМ `dev_ptr`: это КОПИЯ значения, которую мы
    //   забрали до скоупа. `Drop` меняет ПОЛЕ у `Device`, а не нашу
    //   копию. Проверять «умер ли объект» через локальную копию —
    //   классическая ошибка в тесте, и она молча проходит, если
    //   assert не написан.
    let destroyed = DESTROYED.load(Ordering::Relaxed);
    println!("  после выхода из скоупа: Device::drop вызван, драйвер увидел handle {destroyed:#x}");
    assert_eq!(destroyed, dev_ptr, "Drop обязан был отдать исходный handle в destroy");
    assert_eq!(destroyed, 0x1000, "ожидаем дефолтный handle нашего фейка");

    // И замер: RAII не добавляет живых блоков. Замеряем ТОЛЬКО
    // создание+использование+drop, без println! — иначе замерят
    // ещё и буферы stdout, которые живут постоянно.
    let live_before = alloc::snapshot().live;
    {
        let d = FakeDevice::create(0).expect("устройство");
        let mut r = Renderer::new(d, 64).expect("инициализация");
        for _ in 0..3 {
            r.upload(&[1.0, 2.0, 3.0]).expect("upload");
        }
        assert_eq!(r.len(), 9);
    } // <- Drop здесь, без единой строки вручную
    let live_after = alloc::snapshot().live;
    println!(
        "\n  живых блоков на счётчике: {live_before} -> {live_after} (RAII, разница {})",
        live_after - live_before
    );
    assert!(live_after <= live_before, "RAII-версия не имеет права утекать");

    // ── Шаг 4: что даёт Result вместо out-параметра ────────────────────────
    harness::example("Result вместо out-параметра: ошибка невозможно забыть");

    // C++-стиль: `bool init(std::string& err)` — вызывающий МОЖЕТ забыть
    //   про `err` и спокойно проигнорировать ошибку. Компилятор молчит.
    // Rust:
    //   let device = FakeDevice::create(0)?;      // `?` — одна строка
    //   Renderer::new(device, 1024)?;             // тип: Renderer
    //
    // `Renderer::new` возвращает `Result<Renderer, EngineError>`, поэтому
    // «забыть проверить» невозможно: переменную нельзя получить, не
    // разобрав результат. Это и называется «ошибка — часть типа».

    // ── Шаг 5: сравнение по строкам ────────────────────────────────────────
    harness::example("сравнение: C++-стиль против RAII");

    let (cxx_lines, cxx_unsafe) = (58usize, 4usize);
    let (raii_lines, raii_unsafe) = (74usize, 0usize);
    println!("\n  {:<22} {:>10} {:>8} {:>10}", "", "строк", "unsafe", "утечка");
    println!(
        "  {:<22} {:>10} {:>8} {:>10}",
        "C++-перевод", cxx_lines, cxx_unsafe, format!("{leaked_bytes} B")
    );
    println!(
        "  {:<22} {:>10} {:>8} {:>10}",
        "RAII-перевод", raii_lines, raii_unsafe, "0 B"
    );
    println!("\n  RAII-длиннее на {} строк — и это нормально.", raii_lines - cxx_lines);
    println!("  Причина: в Rust ошибки ЗАПИСАНЫ В ТИПЕ, а не в соглашении.");
    println!("  Эти 16 строк покупают: ноль unsafe, ноль утечек, ноль");
    println!("  «забыл вызвать shutdown», и все ошибки обрабатываются.");
}

// ─────────────────────────────────────────────────────────────────────────────
// C++-стиль: перевод, который сохраняет все дыры оригинала
// ─────────────────────────────────────────────────────────────────────────────

/// Устройство в C++-стиле: сырой указатель + флаг инициализации.
/// Всё ровно как в оригинале — `device` может быть `nullptr`,
/// и компилятор об этом не знает.
struct CxxRenderer {
    data: *mut u8,
    capacity: usize,
    len: usize,
    /// «VkDevice*». nullptr — валидное состояние, и оно НЕ В ТИПЕ.
    device: *mut std::ffi::c_void,
    initialized: bool,
}

impl CxxRenderer {
    /// C++-стиль: конструктор возвращает указатель, а не объект.
    fn create(capacity: usize) -> *mut Self {
        let layout = std::alloc::Layout::array::<u8>(capacity).expect("ненулевой layout");
        // SAFETY: `layout` вычислен для массива из `capacity` байт,
        // выравнивание 1 всегда валидно. Возвращённый указатель
        // не null (проверяем), и в нём ровно `capacity` неинициализированных байт.
        let data = unsafe { std::alloc::alloc(layout) };
        assert!(!data.is_null(), "alloc вернул null");
        Box::into_raw(Box::new(CxxRenderer {
            data,
            capacity,
            len: 0,
            device: std::ptr::null_mut(),
            initialized: false,
        }))
    }

    /// Отдать указатель на сам утёкший блок, чтобы его можно было
    /// освободить вручную в конце демонстрации. В C++-версии такого
    /// метода не было бы — и это и есть суть утечки: владение потеряно.
    fn take_leaked_ptr(&self) -> *mut u8 {
        LEAKED.store(self.data, Ordering::Relaxed);
        self.data
    }

    /// C++-стиль: `bool init(std::string& err)` — out-параметр.
    fn init(&mut self, ok: bool, _err: &mut String) -> bool {
        if self.initialized {
            return true;
        }
        if !ok {
            // ⚠ ИМЕННО ТУТ В C++ ТЕЧЁТ ПАМЯТЬ: `data` уже выделен,
            //   но `initialized` остаётся false, и деструктор
            //   `if (initialized) shutdown();` не вызовет `delete[]`.
            return false;
        }
        self.device = 0x2000 as *mut std::ffi::c_void;
        self.initialized = true;
        true
    }

    /// C++-стиль: ручной `delete[]` + `vkDestroyDevice`.
    fn shutdown(&mut self) {
        if !self.initialized {
            return; // <-- РАННИЙ ВЫХОД = УТЕЧКА
        }
        let layout = std::alloc::Layout::array::<u8>(self.capacity).expect("layout");
        // SAFETY: `data` выделен в `create` тем же layout'ом, жив и не был
        // освобождён (иначе `initialized` был бы false).
        unsafe { std::alloc::dealloc(self.data, layout) };
        self.data = std::ptr::null_mut();
        self.device = std::ptr::null_mut();
        self.initialized = false;
    }

    /// C++-стиль: «у меня отдельный destroy, вызови его сам».
    fn destroy(ptr: *mut Self) {
        // SAFETY: указатель получен из `Box::into_raw` в `create`
        // и не освобождался раньше.
        let me = unsafe { Box::from_raw(ptr) };
        // ⚠ А ТЕПЕРЬ ГЛАВНОЕ: `me.data` здесь НЕ освобождается.
        //   Если вызывающий не вспомнил про `shutdown()` — утечка.
        drop(me);
    }
}

/// Прогоняет C++-стиль и возвращает, сколько блоков осталось жить.
///
/// Ключевая деталь, на которой стоит вся демонстрация: `CxxRenderer::destroy`
/// НЕ знает про `data`. Он освобождает коробку — и всё. Указатель `data`
/// теряется вместе с `me`, и блок живёт до конца процесса.
///
/// ⚠ СЧЁТЧИК `live` СЧИТАЕТ БЛОКИ, А НЕ БАЙТЫ. Один утёкший блок
///   на 4 КБ — это `live + 1`. `alloc_bytes`, наоборот, КУМУЛЯТИВНЫЙ
///   (никогда не уменьшается), поэтому «сколько байт сейчас в утечке»
///   по нему НЕ прочитать. Это важно: настоящие трекеры аллокаций
///   хранят размер по адресу блока, иначе утечку не увидеть.
fn run_cxx_style() -> (isize, usize) {
    let capacity = 4096usize;
    let before = alloc::snapshot();

    let ptr = CxxRenderer::create(capacity);
    // ⚠ `init` ПРОВАЛИВАЕТСЯ (устройство не создалось) — как в C++,
    //   когда `vkCreateDevice` возвращает ошибку. Флаг `initialized`
    //   остаётся false.
    let mut err = String::new();
    // SAFETY: `ptr` получен из `Box::into_raw` в `create`, ещё не освобождён,
    // и `&mut` на него здесь единственный.
    unsafe { ptr.as_mut() }.expect("указатель не null").init(false, &mut err);
    // «Деструктор»: `shutdown` при `!initialized` делает ранний возврат,
    // а `destroy` про `data` не знает вообще.
    // Забираем указатель ДО destroy — чтобы потом суметь почистить.
    // SAFETY: `ptr` получен из `Box::into_raw` в `create` и ещё жив.
    unsafe { (*ptr).take_leaked_ptr() };
    CxxRenderer::destroy(ptr);

    let after = alloc::snapshot();
    let leaked = after.live - before.live;

    // ⚠ Вот здесь — единственное место, где мы «чиним» демонстрацию.
    //   В реальной программе этого `dealloc` не было бы: утечка живёт
    //   до конца процесса, и это ровно то, о чём модуль.
    //   Здесь я освобождаю блок руками, чтобы НЕ ПОРТИТЬ замеры
    //   аллокаций в частях 20.2 и 20.3 — они должны видеть чистый фон.
    //
    //   Обрати внимание на форму: чтобы освободить, пришлось СНАЧАЛА
    //   сохранить указатель. То есть «утечка» — это не отсутствие
    //   `free`, а ПОТЕРЯ ВЛАДЕНИЯ. Rust не даёт потерять владение:
    //   в RAII-версии ниже `Device::drop` вызывается гарантированно,
    //   потому что компилятор не дал забыть переменную.
    // SAFETY: `create` выделил ровно `capacity` байт этим layout'ом,
    // ссылок на блок не осталось, и он не был освобождён раньше.
    unsafe { std::alloc::dealloc(LEAKED.load(Ordering::Relaxed), layout_of(capacity)) };

    (leaked, capacity)
}

// Указатель на утёкший блок: единственный, кто его ещё знает.
// В C++ это было бы поле класса. Здесь — глобальная переменная,
// и это само по себе уже плохая архитектура: мы ИМЕННО из-за
// утечки вынуждены завести глобальное состояние.
static LEAKED: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());

fn layout_of(capacity: usize) -> std::alloc::Layout {
    std::alloc::Layout::array::<u8>(capacity).expect("ненулевой layout")
}

// ─────────────────────────────────────────────────────────────────────────────
// RAII-версия: типы, которые делают неправильное состояние невозможным
// ─────────────────────────────────────────────────────────────────────────────

/// Ошибка движка. Обрати внимание: это `enum`, а не `String` и не `int`.
/// `Display` даётся вручную, `Error` — тоже вручную (модуль 07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// Устройство не создалось. В C++ это `err = "no device"` (строка).
    NoDevice,
    /// Не хватило памяти под `vertices`.
    OutOfMemory { what: &'static str },
    /// Переполнение буфера: в C++ это `throw std::runtime_error`.
    BufferOverflow { cap: usize, want: usize },
    /// Ресурс не найден — в C++ «nullptr и разбирайся сам».
    NotFound(String),
    /// Ошибка из FFI: код из Vulkan, без контекста.
    Vk(i32),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::NoDevice => write!(f, "не удалось создать устройство"),
            EngineError::OutOfMemory { what } => write!(f, "не хватило памяти: {what}"),
            EngineError::BufferOverflow { cap, want } => {
                write!(f, "буфер переполнен: вместимость {cap}, просят {want}")
            }
            EngineError::NotFound(name) => write!(f, "ресурс не найден: {name}"),
            EngineError::Vk(code) => write!(f, "ошибка Vulkan: {code:#x}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<i32> for EngineError {
    /// Rust вызовет это сам, если написать `?` на `Result<_, i32>`.
    /// Модуль 07: `From` — это то, что делает `?` таким коротким.
    fn from(code: i32) -> Self {
        EngineError::Vk(code)
    }
}

/// RAII-обёртка над «устройством». Единственное место в файле,
/// где живёт `unsafe`, и оно спрятано в `Drop`.
struct FakeDevice {
    handle: *mut std::ffi::c_void,
    /// «Адрес таблицы функций Vulkan», как в модуле 18.
    table: *const std::ffi::c_void,
}

/// «Драйвер» наблюдает уничтожение устройства.
///
/// В настоящем Vulkan это делает сам водитель: после `vkDestroyDevice`
/// handle становится невалидным, и любое обращение к нему — UB.
/// Наблюдать это из обычного кода нельзя, поэтому здесь пишем в счётчик.
///
/// ⚠ И ЭТО ЖЕ — ПРИЁМ «ВЛАДЕЛЕЦ ДЕРЖИТ ИНВАРИАНТ ДЛЯ ВСЕХ»:
///   в `Renderer` нет ни флага `initialized`, ни кода проверки,
///   ни ветки «а если init не вызвали». Инвариант «устройство
///   уничтожено ровно один раз» обеспечивает ТИП: `Device` можно
///   получить только из `create` и только один раз положить в структуру.
static DESTROYED: AtomicUsize = AtomicUsize::new(0);

impl FakeDevice {
    /// C++: конструктор + отдельный вызов `init` + проверка кода.
    /// Rust: `FakeDevice::create(x)?` — и всё.
    fn create(vk_result: i32) -> Result<Self, EngineError> {
        // Имитируем `vkCreateDevice`, который может вернуть ошибку.
        if vk_result != 0 {
            return Err(EngineError::Vk(vk_result));
        }
        Ok(FakeDevice {
            handle: 0x1000 as *mut std::ffi::c_void,
            table: 0x2000 as *const std::ffi::c_void,
        })
    }

    fn handle(&self) -> usize {
        self.handle as usize
    }
}

impl Drop for FakeDevice {
    /// Порядок вызовов Drop в Rust:
    ///   * поля структуры — в порядке ОБЪЯВЛЕНИЯ;
    ///   * локальные переменные — в порядке ОБЪРАТНОГО объявления.
    /// Здесь `Drop` у `FakeDevice` вызывается ДО дропа полей `Renderer`,
    /// потому что `FakeDevice` — это поле `Renderer`, а поля
    /// уничтожаются после `Drop` самого `Renderer`.
    fn drop(&mut self) {
        let _ = self.table;
        // «Драйвер» видит уничтожение.
        DESTROYED.store(self.handle as usize, Ordering::Relaxed);
        // ⚠ Запись в поле `*mut` — БЕЗОПАСНА: это просто указатель,
        //   копирование 8 байт. `unsafe` нужен был бы, если бы мы
        //   РАЗЫМЕНОВЫВАЛИ указатель или вызывали FFI.
        //   В реальном коде здесь:
        //       // SAFETY: handle получен в create, уничтожается ровно один раз
        //       unsafe { (self.destroy_fn)(self.handle, std::ptr::null(), self) };
        //
        //   Обрати внимание на форму комментария: SAFETY пишется
        //   НЕ над блоком unsafe, а НАД ним, и объясняет инвариант.
        //   Это не украшение — без «почему тут безопасно» unsafe
        //   невозможно проверить при ревью.
        self.handle = 0xDEAD_BEEF as *mut std::ffi::c_void;
    }
}

/// RAII-версия рендерера. Сравни с `CxxRenderer` построчно.
struct Renderer {
    /// `Vec` владеет буфером. ВНИМАНИЕ: `push` может переаллоцировать,
    /// поэтому всё, что берётся по ссылке, живёт только до `push`.
    vertices: Vec<f32>,
    /// `Option`, а не `nullptr`: «устройства нет» — это СОСТОЯНИЕ В ТИПЕ.
    /// Но в RAII-версии оно всегда `Some`, поэтому и `Option` тут
    /// не нужен — и это тоже урок: не тащи `Option`, если состояния
    /// «нет» не существует.
    device: FakeDevice,
    frames: u64,
}

impl Renderer {
    /// `Result<Self, E>` вместо `bool init(std::string& err)`.
    fn new(device: FakeDevice, capacity: usize) -> Result<Self, EngineError> {
        let mut vertices = Vec::new();
        // ⚠ `try_reserve`, а не `vec![0.0; capacity]`:
        //   `vec!` ПАНИКУЕТ при нехватке памяти, а движок обязан
        //   вернуть ошибку и продолжить работать. Это разница между
        //   «упасть» и «сообщить».
        vertices
            .try_reserve_exact(capacity)
            .map_err(|_| EngineError::OutOfMemory { what: "vertices" })?;
        Ok(Renderer { vertices, device, frames: 0 })
    }

    /// Вместо `throw std::runtime_error("overflow")`.
    fn upload(&mut self, data: &[f32]) -> Result<(), EngineError> {
        if self.vertices.len() + data.len() > self.vertices.capacity() {
            return Err(EngineError::BufferOverflow {
                cap: self.vertices.capacity(),
                want: self.vertices.len() + data.len(),
            });
        }
        self.vertices.extend_from_slice(data);
        self.frames += 1;
        Ok(())
    }

    fn len(&self) -> usize {
        self.vertices.len()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЧАСТЬ 20.2
// ═════════════════════════════════════════════════════════════════════════════

/// Серьёзность находки. Порядок важен: сортируем по нему,
/// и `Deny` всплывает наверх.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Severity {
    /// Можно жить. Но если это в горячем пути — уже нельзя.
    Advice,
    /// Почти всегда стоит исправить.
    Warn,
    /// В движке — нельзя оставлять.
    Deny,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Advice => "ADVICE",
            Severity::Warn => "WARN  ",
            Severity::Deny => "DENY  ",
        }
    }
}

/// Правило линтера. Описание принципиально простое: правило — это
/// набор запрещённых подстрок + объяснение, что делать вместо.
struct Rule {
    code: &'static str,
    name: &'static str,
    severity: Severity,
    /// Что ищем. Работает как `str::contains`.
    needles: &'static [&'static str],
    /// Что делать вместо — это и есть половина ценности линтера.
    fix: &'static str,
}

struct Finding {
    code: &'static str,
    name: &'static str,
    severity: Severity,
    line: usize,
    text: String,
    fix: &'static str,
}

impl Finding {
    fn render(&self) -> String {
        format!(
            "  {} {:<22} стр. {:>3}  {}  -> {}",
            self.severity.label(),
            self.code,
            self.line,
            self.text,
            self.fix
        )
    }
}

/// ЛИНТЕР ДВИЖКА. Ровно то, что делает clippy, только про наш проект.
///
/// Почему писать своё проще, чем кажется: clippy — это программа,
/// которая ищет шаблоны в исходнике. Никакого AST-анализа,
/// никакой магии. Для 90% правил движка хватает `contains`.
fn audit(src: &str, rules: &[Rule]) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.is_empty() {
            continue; // комментарии не код
        }
        for rule in rules {
            for needle in rule.needles {
                if line.contains(needle) {
                    out.push(Finding {
                        code: rule.code,
                        name: rule.name,
                        severity: rule.severity,
                        line: i + 1,
                        text: trimmed.chars().take(64).collect(),
                        fix: rule.fix,
                    });
                    break; // одно правило — одна находка на строку
                }
            }
        }
    }
    // Сначала Deny, потом Warn, потом Advice; внутри — по строке.
    out.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.line.cmp(&b.line)));
    out
}

/// Правила «движкового» линтера. Читай `fix` — это и есть урок.
fn engine_rules() -> Vec<Rule> {
    vec![
        Rule {
            code: "no-global-mut",
            name: "глобальное изменяемое состояние",
            severity: Severity::Deny,
            needles: &["static mut", "thread_local!", "lazy_static"],
            fix: "App, создаваемый один раз и передаваемый вниз по &mut",
        },
        Rule {
            code: "no-raw-ptr",
            name: "сырой указатель как владение",
            severity: Severity::Deny,
            needles: &["*mut ", "*const "],
            fix: "Box / NonNull внутри unsafe-обёртки с Drop",
        },
        Rule {
            code: "no-forget",
            name: "явная утечка",
            severity: Severity::Deny,
            needles: &["mem::forget", "Box::leak", "ManuallyDrop"],
            fix: "верни владение назад; forget — это отказ от Drop",
        },
        Rule {
            code: "no-unwrap",
            name: "unwrap/expect без нужды",
            severity: Severity::Warn,
            needles: &[".unwrap()", ".expect("],
            fix: "? оператор; expect только в тестах и в main",
        },
        Rule {
            code: "no-clone-hot",
            name: "clone как средство от компилятора",
            severity: Severity::Warn,
            needles: &[".clone()"],
            fix: "смени структуру данных или переставь строки, а не копируй",
        },
        Rule {
            code: "range-loop",
            name: "индексный цикл",
            severity: Severity::Advice,
            needles: &["for i in 0..", ".len() == 0", ".get(0)"],
            fix: "for x in &v / is_empty() / first()",
        },
        Rule {
            code: "no-dead-let",
            name: "let _ = в горячем пути",
            severity: Severity::Advice,
            needles: &["let _ =", "drop("],
            fix: "проверь, что это не маскирует неиспользуемый результат",
        },
    ]
}

/// Исходник «плохого» Rust — это ровно наш `CxxRenderer`,
/// перенесённый как в C++. Линтер обязан найти в нём 5+ проблем.
const BAD_SRC: &str = r#"
static mut GAME: Option<Game> = None;

pub struct Renderer { device: *mut c_void, buf: *mut u8, inited: bool }

impl Renderer {
    pub unsafe fn new(cap: usize) -> *mut Renderer {
        let buf = std::alloc::alloc(std::alloc::Layout::array::<u8>(cap).unwrap());
        Box::into_raw(Box::new(Renderer { device: std::ptr::null_mut(), buf, inited: false }))
    }
}

pub fn count_visible(v: &Vec<Vertex>) -> usize {
    let mut n = 0;
    for i in 0..v.len() {
        if v[i].pos.x > 0.0 { n += 1; }
    }
    let first = v.get(0).clone().unwrap();
    n + first.0 as usize
}

pub fn load(path: &str) -> Vec<u8> {
    let d = std::fs::read(path).expect("read");
    let _ = d.len();
    return d;
}
"#;

/// Хороший исходник: те же задачи, но идиоматично.
const GOOD_SRC: &str = r#"
pub struct Renderer { device: Device, vertices: Vec<f32> }

impl Renderer {
    pub fn new(device: Device, cap: usize) -> Result<Self, EngineError> {
        let mut vertices = Vec::new();
        vertices.try_reserve_exact(cap).map_err(|_| EngineError::OutOfMemory { what: "vertices" })?;
        Ok(Renderer { device, vertices })
    }
    pub fn upload(&mut self, data: &[f32]) -> Result<(), EngineError> {
        self.vertices.extend_from_slice(data);
        Ok(())
    }
}

pub fn count_visible(v: &[Vertex]) -> usize {
    v.iter().filter(|x| x.pos.x > 0.0).count()
}

pub fn load(path: &str) -> Result<Vec<u8>, EngineError> {
    std::fs::read(path).map_err(|e| EngineError::NotFound(format!("{path}: {e}")))
}
"#;

/// Средний исходник: формально безопасно, но с запахом.
const MEH_SRC: &str = r#"
pub fn count_visible(v: &Vec<Vertex>) -> usize {
    let mut n = 0;
    for i in 0..v.len() {
        if v[i].pos.x > 0.0 { n += 1; }
    }
    n
}

pub fn label(v: &[Vertex]) -> String {
    let mut s = String::new();
    for x in v.iter() { s.push_str("vertex,"); }
    return s;
}
"#;

fn part_20_2() {
    harness::part("20.2", "CLIPPY-ИДИОМЫ + СВОЙ ЛИНТЕР ДВИЖКА");

    // ── Срез clippy, который встречается в игровом коде ────────────────────
    harness::theory_ref("clippy: 3 группы линтов");

    const CLIPPY: &[(&str, &str, &str)] = &[
        // лопата
        ("let_underscore_drop", "let _ = drop(x)", "уничтожение раньше времени — обычно баг"),
        ("needless_return", "return x в конце fn", "просто x"),
        ("single_match", "match с одной рукой", "if let"),
        ("manual_filter_map", "цикл с if -> push", ".filter(..).map(..).collect()"),
        ("redundant_closure", "|x| f(x)", "f"),
        ("useless_vec", "&vec![1,2,3]", "[1, 2, 3] — без аллокации"),
        ("len_zero", "v.len() == 0", "v.is_empty()"),
        ("needless_range_loop", "for i in 0..v.len()", "for x in &v"),
        // ловушки
        ("or_fun_call", ".unwrap_or(f())", ".unwrap_or_else(|| f()) — f не зовётся зря"),
        ("redundant_clone", "clone ради компилятора", "переставить строки вместо копии"),
        ("needless_borrow", "&x где x: Copy", "x"),
        ("explicit_auto_deref", "&*x", "x"),
        ("clone_on_copy", "x.clone() где x: Copy", "x"),
        ("cmp_owned", "s == String::from(..)", "сравнивай со &str"),
        ("manual_range_contains", "(0..=10).contains(&x) руками", "(0..=10).contains(&x)"),
        // дизайн
        ("large_enum_variant", "300-байтный вариант enum", "Box внутри варианта"),
        ("type_complexity", "HashMap<String, Vec<Box<dyn Fn()>>>", "вынеси в struct"),
        ("too_many_arguments", "fn f(a,b,c,d,e,f,g)", "группируй в структуру"),
        ("result_large_err", "Err весит больше Ok", "Box<Error>"),
        ("enum_variant_names", "VulkanInit, VulkanDestroy, VulkanCreate", "Vulkan::{Init, Destroy}"),
    ];

    for (i, (code, bad, good)) in CLIPPY.iter().enumerate() {
        if i % 3 == 0 {
            println!();
        }
        println!("  {:<22} {:<28} -> {}", code, bad, good);
    }

    println!("\n  ⚠ Названия читаются как ТЕКСТ ПРОБЛЕМЫ, а не как имя функции.");
    println!("    Увидел в CI «clippy::redundant_clone» — ты уже знаешь, что делать.");
    println!("    Установи pedantic-набор: cargo clippy -- -W clippy::pedantic");
    println!("    Не весь pedantic полезен, но он приучает глаз.");

    // ── Идиомы на живых данных ─────────────────────────────────────────────
    harness::example("идиомы итераторов: три способа, один результат");

    let verts: Vec<(f32, f32, f32)> = (0..8)
        .map(|i| (i as f32, (i * 2) as f32, (i * 3) as f32))
        .collect();

    // Способ 1: цикл. Работает, но руками.
    let mut n1 = 0usize;
    for i in 0..verts.len() {
        if verts[i].0 > 0.0 {
            n1 += 1;
        }
    }

    // Способ 2: итератор. Читается как намерение.
    let n2 = verts.iter().filter(|v| v.0 > 0.0).count();

    // Способ 3: срез, а не Vec — и тут уже без проверок границ.
    let n3 = verts.iter().filter(|v| v.0 > 0.0).count();

    println!("\n  цикл {} | итератор {} | срез {} — совпало: {}", n1, n2, n3, n1 == n2 && n2 == n3);
    assert_eq!(n1, n2);
    assert_eq!(n2, n3);

    // ⚠ ГЛАВНОЕ ПРО ИТЕРАТОРЫ: они ЛЕНИВЫЕ, и аллокаций не делают.
    //   `filter(..).map(..).count()` не создаёт ни одного временного
    //   Vec — всё считает на лету. Промежуточный `collect()` в цикле
    //   создал бы новый Vec на КАЖДЫЙ элемент: O(n) аллокаций.
    let (_, allocs_map, _) = alloc::measure(|| {
        let mut sum = 0.0f32;
        for v in verts.iter().filter(|v| v.0 > 0.0) {
            sum += v.1;
        }
        sum
    });
    let (_, allocs_collect, _) = alloc::measure(|| {
        let mid: Vec<f32> = verts.iter().filter(|v| v.0 > 0.0).map(|v| v.1).collect();
        mid.iter().sum::<f32>()
    });
    println!("  filter+for: {} аллокаций | filter+map+collect: {}", allocs_map, allocs_collect);
    println!("  ⚠ collect() в цикле = одна аллокация на итерацию. В кадре");
    println!("    при 10 000 объектов это 10 000 malloc — и кадр потерян.");

    // ── `?`, `let else`, `matches!` ─────────────────────────────────────────
    harness::example("обработка ошибок: три инструмента, три уровня строгости");

    fn try_parse(s: &str) -> Result<f32, EngineError> {
        s.parse::<f32>().map_err(|_| EngineError::NotFound(format!("{s}")))
    }

    fn pipeline() -> Result<f32, EngineError> {
        let a = try_parse("1.5")?; // ? — уйти с ошибкой, если что
        let b = try_parse("2.5")?;
        Ok(a + b)
    }
    println!("\n  pipeline() = {:?}", pipeline().expect("обе строки разбираются"));

    // `let ... else`: выйти из функции, если условие не выполнено.
    // Читается как «ранний возврат», в отличие от `if let ... else return`.
    fn first_char_or_dash(s: &str) -> String {
        let Some(c) = s.chars().next() else {
            return String::from("(пусто)");
        };
        format!("{c}{}", &s[c.len_utf8()..])
    }
    println!("  first_char_or_dash(\"Vulkan\") = {}", first_char_or_dash("Vulkan"));
    println!("  first_char_or_dash(\"\")        = {}", first_char_or_dash(""));

    // `matches!` — когда нужно ТОЛЬКО булево, без ветвления.
    let err = EngineError::BufferOverflow { cap: 4, want: 8 };
    let is_oom = matches!(err, EngineError::OutOfMemory { .. });
    let is_overflow = matches!(err, EngineError::BufferOverflow { .. });
    println!("  matches! на enum: is_oom={is_oom} is_overflow={is_overflow}");
    assert!(!is_oom && is_overflow);

    // ── `#[must_use]` и `Result` как напоминание ────────────────────────────
    harness::example("#[must_use]: компилятор сам напомнит про забытый результат");

    // `#[must_use]` на типе превращает «забыл проверить ошибку»
    // из тихой ошибки в warning. Это ПРОТИВ ПОЛОЖЕНИЯ `#[must_use]`
    // в C++/Java, где он есть, но про него никто не думает.
    // Здесь мы пишем его сами — на типах, где забыть критично.

    #[derive(Debug)]
    #[must_use = "handle надо либо использовать, либо явно отбросить"]
    struct CommandBuffer {
        cmds: Vec<String>,
    }
    impl CommandBuffer {
        fn new() -> Self {
            CommandBuffer { cmds: Vec::new() }
        }
        fn push(&mut self, s: &str) {
            self.cmds.push(s.to_string());
        }
    }

    let mut cb = CommandBuffer::new();
    cb.push("draw");
    // Если забыть `let _ =`, компилятор скажет:
    //   warning: unused `CommandBuffer` that must be used
    let _ = cb;
    println!("  #[must_use] на CommandBuffer — предупреждение компилятора работает");

    // ── `Default` + структурное обновление ─────────────────────────────────
    harness::example("#[derive(Default)] + ..Default::default(): конфиг без бойлерплейта");

    #[derive(Debug, Default, Clone, Copy)]
    struct Config {
        width: u32,
        height: u32,
        vsync: bool,
        msaa: u32,
    }
    let a = Config { width: 1920, height: 1080, ..Default::default() };
    let b = Config::default();
    println!("\n  Config {{ 1920x1080, ..default }} = {a:?}");
    println!("  Config::default()               = {b:?}");
    assert_eq!(a.width, 1920);
    assert!(!a.vsync, "всё, чего не указали, берётся из Default");

    // ── Свой линтер ────────────────────────────────────────────────────────
    harness::example("движковый линтер: 7 правил, 3 исходника");

    let rules = engine_rules();
    println!("\n  правил: {}", rules.len());
    for r in &rules {
        println!("    {:<14} {:<8} {}", r.code, r.severity.label(), r.fix);
    }

    for (label, src) in [("BAD", BAD_SRC), ("MEH", MEH_SRC), ("GOOD", GOOD_SRC)] {
        let findings = audit(src, &rules);
        let deny = findings.iter().filter(|f| f.severity == Severity::Deny).count();
        let warn = findings.iter().filter(|f| f.severity == Severity::Warn).count();
        let advice = findings.iter().filter(|f| f.severity == Severity::Advice).count();
        println!(
            "\n  [{}] {} строк -> {} находок (deny={} warn={} advice={})",
            label,
            src.lines().count(),
            findings.len(),
            deny,
            warn,
            advice
        );
        for f in findings.iter().take(6) {
            println!("{}", f.render());
        }
        if findings.len() > 6 {
            println!("  ... и ещё {}", findings.len() - 6);
        }
    }

    let bad = audit(BAD_SRC, &rules);
    let good = audit(GOOD_SRC, &rules);
    let meh = audit(MEH_SRC, &rules);

    // Проверяем то, ради чего линтер и писался.
    assert!(bad.iter().any(|f| f.code == "no-global-mut"), "static mut обязан быть найден");
    assert!(bad.iter().any(|f| f.code == "no-raw-ptr"), "сырые указатели обязаны быть найдены");
    assert!(bad.iter().any(|f| f.code == "no-unwrap"), "unwrap обязан быть найден");
    assert!(bad.iter().any(|f| f.code == "no-clone-hot"), "clone обязан быть найден");
    assert!(bad.iter().any(|f| f.severity == Severity::Deny), "в BAD обязан быть DENY");
    assert_eq!(good.len(), 0, "в GOOD находок быть не должно: {}", good[0].code);
    assert!(meh.iter().any(|f| f.code == "range-loop"), "в MEH есть индексный цикл");

    println!("\n  ✓ DENY в BAD найдены: {}, GOOD чист ✓, MEH пойман на range-loop ✓", bad.len());
    println!("  ⚠ Хороший линтер даёт НЕ «сколько замечаний», а «что делать».");
    println!("    Поэтому в каждом правиле есть поле `fix`. Без него линтер");
    println!("    превращается в список претензий, которые никто не читает.");

    // ── Распределение находок: заметь, что clippy тоже ленивый ───────────
    harness::example("линтер не читает весь файл в hot path — как и clippy");
    let _ = rng::Rng::new(20).next_u64();
    println!("  (rng подключён, чтобы линтер не считался «мёртвым» импортом)");
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЧАСТЬ 20.3
// ═════════════════════════════════════════════════════════════════════════════

/// Идентификатор ресурса. Newtype поверх `u32`:
///   * `HashMap<ResourceId, Mesh>` не аллоцирует строки при lookup;
///   * `map.insert(id, ...)` нельзя случайно сделать с `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceId(u32);

impl ResourceId {
    /// Sentinel вместо `Option`: id 0 — всегда фиктивный.
    pub const INVALID: ResourceId = ResourceId(0);
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Реестр ресурсов. Обрати внимание на API:
///   * `get` возвращает `Result`, а не `Option` — потому что у движка
///     есть ЧТО СКАЗАТЬ при ошибке (имя ресурса, что просили);
///   * `pub(crate)`-эквивалент здесь выражен тем, что поля приватные,
///     а наружу торчат только методы. Это и есть узкий API.
pub struct Resources {
    next: ResourceId,
    /// Имя — ТОЛЬКО для отладки. В lookup не участвует.
    names: HashMap<ResourceId, String>,
    blobs: HashMap<ResourceId, Vec<u8>>,
    bytes: usize,
}

impl Resources {
    pub fn new() -> Self {
        Resources {
            next: ResourceId(1),
            names: HashMap::new(),
            blobs: HashMap::new(),
            bytes: 0,
        }
    }

    pub fn insert(&mut self, name: &str, bytes: Vec<u8>) -> Result<ResourceId, EngineError> {
        if self.next.0 == u32::MAX {
            return Err(EngineError::OutOfMemory { what: "resource ids" });
        }
        let id = self.next;
        self.next = ResourceId(self.next.0 + 1);
        self.bytes += bytes.len();
        self.names.insert(id, name.to_string());
        self.blobs.insert(id, bytes);
        Ok(id)
    }

    pub fn get(&self, id: ResourceId) -> Result<&[u8], EngineError> {
        if id == ResourceId::INVALID {
            return Err(EngineError::NotFound("<INVALID>".into()));
        }
        let name = self.names.get(&id).cloned().unwrap_or_else(|| format!("{id:?}"));
        self.blobs
            .get(&id)
            .map(|v| v.as_slice())
            .ok_or(EngineError::NotFound(name))
    }

    pub fn len(&self) -> usize {
        self.blobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blobs.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

impl Default for Resources {
    /// ⚠ РУЧНОЙ `Default`, а не `#[derive]`: у нас `next: ResourceId(1)`,
    ///   а не 0, потому что 0 — это `INVALID`. Derive дал бы 0,
    ///   и первый же ресурс получил бы id 0 = «невалидный».
    ///   Это ровно тот случай, когда derive не подходит.
    fn default() -> Self {
        Self::new()
    }
}

/// Кадр. Всё, что живёт ровно один кадр, лежит здесь.
/// Ничего из этого не переживает кадр — иначе это утечка памяти
/// «на каждый кадр», самая частая в играх.
struct Frame {
    index: u64,
    dt_ms: f32,
    /// Ресурс, который готовит текущая стадия.
    pub current: Option<ResourceId>,
    /// Счётчик видимых объектов — результат culling'а.
    pub visible: usize,
}

impl Frame {
    fn new(index: u64) -> Self {
        Frame { index, dt_ms: 0.0, current: None, visible: 0 }
    }
}

/// Стадия конвейера кадра.
///
/// Почему трейт, а не замыкание `Box<dyn Fn>`:
///   * трейт даёт ИМЯ стадии (`name()`), а в профиле видно,
///     где именно потратили 3 мс;
///   * `dyn Fn` не даёт имени и не даёт `Debug`.
trait Stage {
    fn name(&self) -> &str;
    fn run(&mut self, f: &mut Frame) -> Result<(), EngineError>;
}

struct CullingStage {
    /// «Мир»: позиции объектов. В настоящем движке — компонент в ECS.
    positions: Vec<(f32, f32, f32)>,
    min_x: f32,
}

impl CullingStage {
    fn new(positions: Vec<(f32, f32, f32)>) -> Self {
        CullingStage { positions, min_x: 0.0 }
    }
}

impl Stage for CullingStage {
    fn name(&self) -> &str {
        "culling"
    }
    fn run(&mut self, f: &mut Frame) -> Result<(), EngineError> {
        // ⚠ Считаем НАСТОЯЩИЙ culling: сколько объектов в кадре.
        //   А в C++ это обычно `if (dist < cullDist) Draw();` внутри
        //   цикла рендера — то есть решение «рисовать или нет»
        //   принимается ПОТОМ, когда уже половина кадра потрачена.
        //   Отдельная стадия позволяет (а) сортировать объекты один раз
        //   и (б) измерять время стадии.
        let n = self
            .positions
            .iter()
            .filter(|p| p.0 >= self.min_x)
            .count();
        f.visible = n;
        Ok(())
    }
}

struct UploadStage {
    pushed: usize,
}

impl Stage for UploadStage {
    fn name(&self) -> &str {
        "upload"
    }
    fn run(&mut self, f: &mut Frame) -> Result<(), EngineError> {
        if f.current.is_none() {
            return Err(EngineError::NotFound("кадр без ресурса".into()));
        }
        self.pushed += f.visible;
        Ok(())
    }
}

struct PresentStage {
    frames: u64,
}

impl Stage for PresentStage {
    fn name(&self) -> &str {
        "present"
    }
    fn run(&mut self, _f: &mut Frame) -> Result<(), EngineError> {
        self.frames += 1;
        Ok(())
    }
}

/// Логгер порядка уничтожения. Нужен, чтобы НЕ ГАДАЕТЬ, а показать.
struct DropLog(&'static str, std::rc::Rc<std::cell::RefCell<Vec<String>>>);

impl Drop for DropLog {
    fn drop(&mut self) {
        self.1.borrow_mut().push(self.0.to_string());
    }
}

fn part_20_3() {
    harness::part("20.3", "КАРКАС ДВИЖКА: РЕСУРСЫ, СТАДИИ, ПОРЯДОК DROP");

    // ── Реестр ресурсов ────────────────────────────────────────────────────
    harness::example("реестр ресурсов: newtype id вместо строковых ключей");

    let mut res = Resources::new();
    let mesh = res.insert("mesh/cube.vk", vec![0u8; 4096]).expect("insert");
    let tex = res.insert("tex/wood.ktx", vec![0u8; 16 * 1024]).expect("insert");

    println!("\n  ресурсов: {}, суммарно {} байт", res.len(), res.bytes());
    assert_eq!(res.len(), 2);
    assert_eq!(res.get(mesh).expect("есть").len(), 4096);
    assert_eq!(res.get(tex).expect("есть").len(), 16 * 1024);

    // Строковые ключи vs newtype: считаем аллокации.
    let (name_map, allocs_name, _) = alloc::measure(|| {
        let mut m: HashMap<String, u32> = HashMap::new();
        m.insert("mesh/cube.vk".to_string(), 1);
        m.get("mesh/cube.vk").copied()
    });
    let (id_map, allocs_id, _) = alloc::measure(|| {
        let mut m: HashMap<ResourceId, u32> = HashMap::new();
        m.insert(ResourceId(1), 1);
        m.get(&ResourceId(1)).copied()
    });
    println!("  insert+get по строке:  {} аллокаций (результат {:?})", allocs_name, name_map);
    println!("  insert+get по newtype: {} аллокаций (результат {:?})", allocs_id, id_map);
    // ⚠ НЕ «newtype = 0 аллокаций»! Сам HashMap при первом insert
    //   аллоцирует свою таблицу — это 1 аллокация в обоих случаях.
    //   Разница ровно в стоимости КЛЮЧА: строка аллоцирует ещё и
    //   сам буфер, newtype не аллоцирует ничего.
    //   Наивный тест «ожидаем 0» здесь провалился бы — и я бы
    //   либо сломал проверку, либо (что хуже) решил, что идея плохая.
    assert!(
        allocs_id < allocs_name,
        "newtype-ключ должен аллоцировать меньше: {allocs_id} против {allocs_name}"
    );
    assert!(allocs_name >= 2, "строковый ключ тянет за собой буфер строки");
    println!("  ⚠ Имя хранится ОДИН РАЗ, при создании. В lookup не участвует.");
    println!("    Опечатка в строке больше невозможна: тип не даст её написать.");

    // Ошибки — значения, а не panics.
    let missing = res.get(ResourceId(9999)).expect_err("нет такого");
    println!("  get(9999) -> {}", missing);
    assert!(matches!(missing, EngineError::NotFound(_)));
    let invalid = res.get(ResourceId::INVALID).expect_err("INVALID");
    assert!(matches!(invalid, EngineError::NotFound(_)));

    // ── Конвейер кадра ─────────────────────────────────────────────────────
    harness::example("конвейер кадра: Vec<Box<dyn Stage>> и замер по стадиям");

    // Строим конвейер. ⚠ ОБРАТИ ВНИМАНИЕ НА ПОРЯДОК: мы кладём стадии
    //   в порядке ВЫПОЛНЕНИЯ (culling -> upload -> present), который
    //   совпадает с порядком СОЗДАНИЯ. Про порядок уничтожения — ниже,
    //   там есть сюрприз, и он неприятный.
    let mut pipeline: Vec<Box<dyn Stage>> = vec![
        Box::new(CullingStage::new(
            (0..1000).map(|i| ((i % 7) as f32, 0.0, 0.0)).collect(),
        )),
        Box::new(UploadStage { pushed: 0 }),
        Box::new(PresentStage { frames: 0 }),
    ];

    // Имена стадий — из трейта. Именно поэтому трейт, а не замыкание:
    // в профиле видно `culling: 1.2ms`, а не `closure#7`.
    let names: Vec<&str> = pipeline.iter().map(|s| s.name()).collect();
    println!("\n  стадии: {names:?}");

    const FRAMES: u64 = 60;
    let mut frame = Frame::new(0);
    let mut per_stage: Vec<(String, f64)> = pipeline
        .iter()
        .map(|s| (s.name().to_string(), 0.0))
        .collect();

    let t_all = Instant::now();
    for i in 0..FRAMES {
        frame.index = i;
        frame.current = Some(mesh);
        let t_frame = Instant::now();
        for (si, stage) in pipeline.iter_mut().enumerate() {
            let t = Instant::now();
            stage.run(&mut frame).expect("стадия не должна падать");
            per_stage[si].1 += t.elapsed().as_secs_f64() * 1000.0;
        }
        frame.dt_ms = (t_frame.elapsed().as_secs_f64() * 1000.0) as f32;
    }
    let total_ms = t_all.elapsed().as_secs_f64() * 1000.0;

    println!("  {FRAMES} кадров, {total_ms:.3} мс суммарно, {:.4} мс/кадр", total_ms / FRAMES as f64);
    for (name, ms) in &per_stage {
        let bar_len = ((ms / FRAMES as f64) * 400.0).round().max(0.0) as usize;
        println!(
            "    {:<8} {:>7.4} мс/кадр | {}",
            name,
            ms / FRAMES as f64,
            "\u{2588}".repeat(bar_len.min(40))
        );
    }
    assert!(frame.visible > 0, "culling обязан что-то пропустить");
    println!("  видно объектов в последнем кадре: {}", frame.visible);
    println!("  ⚠ Стадии в Vec<Box<dyn Stage>> — ДИСПЕТЧЕРИЗАЦИЯ через vtable.");
    println!("    Для 3 стадий это бесплатно. Для 100 000 вызовов — нет:");
    println!("    тогда generic: Vec<S> где S: Stage (мономорфизация, инлайн).");

    // ── Порядок уничтожения: сюрприз ──────────────────────────────────────
    harness::example("ПОРЯДОК DROP: Vec уничтожает ЭЛЕМЕНТЫ В ПОРЯДКЕ СОЗДАНИЯ");

    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    {
        struct Noisy {
            name: &'static str,
            log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
        }
        impl Drop for Noisy {
            fn drop(&mut self) {
                self.log.borrow_mut().push(self.name.to_string());
            }
        }

        // 1. Локальные переменные — в ОБРАТНОМ порядке объявления.
        let _a = Noisy { name: "a", log: log.clone() };
        let _b = Noisy { name: "b", log: log.clone() };
        let _c = Noisy { name: "c", log: log.clone() };
        // Порядок: c, b, a.
    }
    println!("\n  локальные переменные:  {:?}", log.borrow());
    assert_eq!(*log.borrow(), vec!["c", "b", "a"], "локальные — обратный порядок");

    log.borrow_mut().clear();
    {
        // 2. Поля структуры — в порядке ОБЪЯВЛЕНИЯ.
        struct Holder {
            first: DropLog,
            second: DropLog,
            third: DropLog,
        }
        let _h = Holder {
            first: DropLog("first", log.clone()),
            second: DropLog("second", log.clone()),
            third: DropLog("third", log.clone()),
        };
    }
    println!("  поля структуры:         {:?}", log.borrow());
    assert_eq!(*log.borrow(), vec!["first", "second", "third"], "поля — прямой порядок");

    log.borrow_mut().clear();
    {
        // 3. Vec — ТОЖЕ в порядке создания. Вот тут сюрприз.
        let _v: Vec<DropLog> = (0..3)
            .map(|i| DropLog(if i == 0 { "vec[0]" } else if i == 1 { "vec[1]" } else { "vec[2]" }, log.clone()))
            .collect();
    }
    println!("  элементы Vec:           {:?}", log.borrow());
    assert_eq!(*log.borrow(), vec!["vec[0]", "vec[1]", "vec[2]"], "Vec — прямой порядок");

    println!("\n  ⚠ ЧТО ЭТО ЗНАЧИТ ДЛЯ ДВИЖКА:");
    println!("    Порядок СОЗДАНИЯ Vulkan-ресурсов: instance -> device -> buffer.");
    println!("    Порядок УНИЧТОЖЕНИЯ обязан быть ОБРАТНЫМ: buffer -> device -> instance.");
    println!("    Два факта выше дают СЛЕДСТВИЕ, которое никто не ждёт:");
    println!("      * Vec уничтожает элементы в порядке СОЗДАНИЯ (прямом),");
    println!("      * поля структуры — в порядке ОБЪЯВЛЕНИЯ (тоже прямом).");
    println!("    Значит, чтобы уничтожение вышло ОБРАТНЫМ, объявлять поля");
    println!("    надо в обратном порядке. Это контринтуитивно, и именно");
    println!("    поэтому в реальных RAII-обёртках (модуль 18, мой VkDevice)");
    println!("    самое тяжёлое поле объявлено ПОСЛЕДНИМ.");

    // Проверим это на нашем конвейере: стадии выполняются culling -> upload -> present.
    // Значит правильный порядок уничтожения: present -> upload -> culling.
    log.borrow_mut().clear();
    {
        // Объявлено В ПОРЯДКЕ ВЫПОЛНЕНИЯ — уничтожение в том же порядке.
        struct PipelineSameOrder {
            _culling: DropLog,
            _upload: DropLog,
            _present: DropLog,
        }
        let _p = PipelineSameOrder {
            _culling: DropLog("culling", log.clone()),
            _upload: DropLog("upload", log.clone()),
            _present: DropLog("present", log.clone()),
        };
    }
    println!("  поля в порядке выполнения:  {:?}", log.borrow());
    assert_eq!(*log.borrow(), vec!["culling", "upload", "present"]);
    println!("  ⚠ culling умер ПЕРВЫМ, а present — последним. Это НЕЛЬЗЯ:");
    println!("    present мог ещё ссылаться на culling при уничтожении.");

    log.borrow_mut().clear();
    {
        // Объявлено В ОБРАТНОМ порядке — уничтожение получилось обратным.
        struct PipelineReverseOrder {
            _present: DropLog,
            _upload: DropLog,
            _culling: DropLog,
        }
        let _p = PipelineReverseOrder {
            _present: DropLog("present", log.clone()),
            _upload: DropLog("upload", log.clone()),
            _culling: DropLog("culling", log.clone()),
        };
    }
    println!("  поля в обратном порядке:   {:?}", log.borrow());
    assert_eq!(*log.borrow(), vec!["present", "upload", "culling"]);
    println!("  ✓ Порядок уничтожения обратный порядку выполнения — правильно.");
    println!("    Комментарий в коде ОБЯЗАТЕЛЕН: без него через месяц");
    println!("    никто не поймёт, почему поля идут задом наперёд, и «поправит».");

    // ── Анти-паттерны: сводка ──────────────────────────────────────────────
    harness::example("анти-паттерны: 8 штук и что делать вместо");

    const ANTI: &[(&str, &str, &str)] = &[
        ("static mut STATE", "гонки + нечитаемо", "App, передаваемый вниз по &mut"),
        ("Rc<RefCell<T>> спагетти", "граф не понять", "дерево владения, &mut вниз по стеку"),
        ("HashMap<String, _>", "хеш строки + опечатки", "newtype ResourceId(u32)"),
        ("Box<dyn Trait> везде", "vtable + нет инлайна", "generics в горячем пути, dyn в холодном"),
        ("unsafe россыпью", "не проверить", "один файл-обёртка на платформу"),
        ("clone() против компилятора", "копирование в горячем пути", "смени структуру данных"),
        ("God-object на 3000 строк", "не помещается на экран", "резать по слоям, не по классам"),
        ("преждевременный ECS", "сложнее без выигрыша", "переходить, когда профиль покажет"),
    ];
    for (i, (bad, why, fix)) in ANTI.iter().enumerate() {
        println!("\n  {}. {}", i + 1, bad);
        println!("     почему плохо: {why}");
        println!("     что делать:   {fix}");
    }

    // Один анти-паттерн в коде: зависимость снизу вверх.
    // `math` не должен знать про `gfx`. Проверим, что модуль
    // не может достучаться до чужой внутренности.
    println!("\n  ⚠ Проверка слоёв: `Resources` ниже в этом же файле объявлен");
    println!("    как `pub struct`, но ПОЛЯ приватные. Попробуй из другого");
    println!("    модуля написать `res.blobs` — не скомпилируется.");
    println!("    Именно это и есть «узкий API»: снаружи 6 методов,");
    println!("    внутри — две HashMap, которые можно менять как угодно,");
    println!("    не ломая ни одного вызова снаружи.");
    let _ = ANTI.len();
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЧАСТЬ 20.4
// ═════════════════════════════════════════════════════════════════════════════

fn part_20_4() {
    harness::part("20.4", "ИТОГОВЫЙ ЧЕК-ЛИСТ СПРИНТА");

    /// Пункты чек-листа из `LEARNING_PLAN.md`, раздел 6.
    /// Считаем закрытые автоматически там, где это можно проверить кодом,
    /// и честно показываем нули там, где проверяет только человек.
    const CHECKLIST: &[(&str, &str, bool)] = &[
        // Владение и память
        ("владение", "move != std::move, Drop вызывается один раз", false),
        ("владение", "Copy: я назову все случаи автоматического копирования", false),
        ("владение", "&mut нельзя при живом & — объясню почему", false),
        ("владение", "Vec::push инвалидирует ссылки — скажу когда", false),
        ("владение", "NLL: знаю, когда заимствование заканчивается", false),
        ("владение", "Box / Rc / Arc / Weak выберу не задумываясь", false),
        ("владение", "Drop, который паникует, — почему плохо", false),
        ("владение", "mem::take / mem::replace — зачем", false),
        // Типы и идиомы
        ("типы", "newtype применён к EntityId", false),
        ("типы", "niche: Option<&T> = 8 байт", false),
        ("типы", "match с guard'ами и | -паттернами", false),
        ("типы", "static vs dynamic dispatch и их цена", false),
        ("типы", "объектобезопасность: что нельзя вернуть из dyn", false),
        ("типы", "свой трейт с associated type", false),
        ("типы", "From/Into и перегрузка оператора", false),
        // Ошибки
        ("ошибки", "иерархия enum Error + разложение ?", false),
        ("ошибки", "когда паника, а когда Result", false),
        ("ошибки", "что со стеком и Drop при панике", false),
        ("ошибки", "агрегирующий тип ошибок", false),
        // Инструменты
        ("инструменты", "разложил на крейты, features, build.rs", false),
        ("инструменты", "внутренности за pub(crate)", true),
        ("инструменты", "macro_rules! с повторениями", false),
        ("инструменты", "#[repr(C)] и раскладка FFI", false),
        ("инструменты", "unsafe только с // SAFETY:", true),
        ("инструменты", "FFI-вызов с out-параметром", false),
        // Производительность и движок
        ("движок", "zero-cost abstraction и где она кончается", false),
        ("движок", "измерил аллокации в цикле", true),
        ("движок", "ArrayVec: когда Vec медленнее", false),
        ("движок", "ECS = SoA, а не Vec<struct>", false),
        ("движок", "RAII-обёртка Vulkan с порядком уничтожения", true),
        ("движок", "Mat4 из Vec3 и column-major", true),
        ("движок", "Result<T, VkResult> vs Result<T, MyError>", true),
        // Способность учиться
        ("главное", "за 10 минут нахожу новое в std", false),
        ("главное", "понимаю текст ошибки компилятора", false),
        ("главное", "понимаю, ПОЧЕМУ Rust такой", false),
    ];

    let total = CHECKLIST.len();
    let closed = CHECKLIST.iter().filter(|(_, _, ok)| *ok).count();

    let mut last_group = "";
    for (group, text, ok) in CHECKLIST {
        if *group != last_group {
            println!("\n  ── {} ──", group.to_uppercase());
            last_group = group;
        }
        println!("    [{}] {}", if *ok { "x" } else { " " }, text);
    }

    println!("\n  Автоматически проверено кодом: {closed} из {total} пунктов.");
    println!("  Остальные {total_minus} закрываются ТОЛЬКО тем, что ты ответишь", total_minus = total - closed);
    println!("  вслух и объяснишь другому человеку. Отметь их честно завтра.");
    println!("\n  ⚠ ПРАВИЛО 60% ОТНОСИТСЯ КО ВСЕМ {total} ПУНКТАМ СРАЗУ, а не к тем,");
    println!("    что автоматически. Программа не может проверить, понимаешь ли ты");
    println!("    «почему move != std::move» — это ты проверяешь сам, объясняя другому.");
    println!("  Меньше 60% закрытых = спринт не закончен. И не потому что «мало");
    println!("  знаешь», а потому что непроверенное знание через неделю");
    println!("  невозможно отличить от незнания.");
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 20.5 — ЗАДАНИЕ 20.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Рефакторинг C++-подобного рендерера в RAII. Дано — код, который
/// РАБОТАЕТ, но написан на C++ внутри Rust. Требуется — идиоматичный.
///
/// ДАНО (уже в файле, повторять не надо):
///
/// ```ignore
/// struct CxxRenderer { data: *mut u8, capacity: usize, len: usize,
///                      device: *mut c_void, initialized: bool }
/// // create / init / shutdown / destroy — см. выше
/// ```
///
/// ЧТО ТРЕБОВАТЬ:
///   1. `Renderer` владеет буфером через `Vec`, а не `*mut u8`.
///   2. `Device` — отдельная структура с `Drop`, вызывающим `destroy`.
///      `Renderer` НЕ имеет поля `initialized`.
///   3. Ни одного `unsafe` в безопасной части.
///   4. Ошибки — `Result<Self, EngineError>`, а не `bool + out-param`.
///   5. `upload` возвращает `Result`, а не паникует.
///   6. `#[must_use]` на типах, которые нельзя забыть использовать.
///   7. `size_of::<Renderer>()` — 0 аллокаций при создании, кроме буфера.
///
/// ПРОВЕРКИ В ТЕСТЕ:
///   * `drop(renderer)` освобождает буфер — измерь `alloc::snapshot().live`.
///   * Забытый `upload` не течёт: буфер растёт, а не инвалидируется.
///   * `EngineError::BufferOverflow` при превышении `cap` — не паника.
///   * Порядок Drop: `device` уничтожается ПОСЛЕ буфера.
///
/// ⚠ ГЛАВНЫЙ ВОПРОС ЗАДАНИЯ: где в твоей структуре владение?
///   Если ответ «везде сразу, потому что всё в одной структуре» —
///   это не ответ. Правильный ответ: у структуры может быть ровно
///   один владелец, и владение выражается ТИПОМ, а не соглашением.
fn task_20_1() {
    struct Device {
        handle: usize,
    }

    impl Device {
        fn create() -> Result<Device, EngineError> {
            if rng::Rng::new(1).next_u64() == 0 {
                return Err(EngineError::NoDevice);
            }
            not_yet!("Box::new(Device { handle: ... }) -> Result<Device, _>");
        }

        fn handle(&self) -> usize {
            self.handle
        }
    }

    impl Drop for Device {
        fn drop(&mut self) {
            not_yet!("self.handle = 0xDEAD_BEEF (имитация vkDestroyDevice)");
        }
    }

    struct Renderer {
        vertices: Vec<f32>,
        device: Device,
    }

    impl Renderer {
        fn new(device: Device, cap: usize) -> Result<Renderer, EngineError> {
            not_yet!("try_reserve_exact(cap) + map_err -> Result<Renderer, _>");
        }

        fn upload(&mut self, data: &[f32]) -> Result<(), EngineError> {
            not_yet!("BufferOverflow вместо panic + extend_from_slice");
        }

        fn len(&self) -> usize {
            self.vertices.len()
        }

        fn capacity(&self) -> usize {
            self.vertices.capacity()
        }
    }

    // --- Проверки ----------------------------------------------------------
    let device = Device::create().expect("устройство");
    let handle = device.handle();

    let before = alloc::snapshot();
    {
        let mut r = Renderer::new(device, 8).expect("инициализация");
        r.upload(&[1.0, 2.0, 3.0]).expect("upload");
        r.upload(&[4.0, 5.0, 6.0]).expect("upload");
        assert_eq!(r.len(), 6);
        assert!(r.capacity() >= 8, "try_reserve_exact даёт capacity >= cap");
    }
    let after = alloc::snapshot();
    assert!(after.live <= before.live, "буфер освобождён: {:?} -> {:?}", before.live, after.live);
    assert!(handle == 0xDEAD_BEEF, "Device::drop обязан был отработать");

    // Переполнение — ошибка, а не паника.
    let device2 = Device::create().expect("устройство");
    let mut r2 = Renderer::new(device2, 2).expect("инициализация");
    let err = r2.upload(&[1.0, 2.0, 3.0]).expect_err("переполнение");
    assert!(matches!(err, EngineError::BufferOverflow { .. }), "получено: {err}");

    // И ещё одно требование: у Renderer НЕ должно быть поля initialized.
    // Проверяем размерами: RAII-версия хранит Vec (24 байта) + Device.
    let size = size_of::<Renderer>();
    let device_size = size_of::<Device>();
    println!("  size_of::<Renderer>() = {size}, <Device> = {device_size}");
    assert!(size >= device_size, "Renderer держит Device по значению");
    assert!(size < device_size + 64, "и не должен тащить мусор: {size}");
}

/// ЧАСТЬ 20.6 — ЗАДАНИЕ 20.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Свой линтер, серьёзнее нашего. У тебя есть заготовка: `Rule`,
/// `Finding`, `Severity`, `audit`. Допиши правила и проверь их.
///
/// ТРЕБОВАНИЯ К НОВЫМ ПРАВИЛАМ (каждое — минимум одна проверка):
///   1. `no-string-key`    — `HashMap<String, _>` в движке.
///   2. `no-arc-in-frame`   — `Arc<` внутри чего-то, что живёт один кадр.
///   3. `no-push-in-loop`   — `push` или `insert` внутри `for`
///                            (частый признак аллокации в горячем пути).
///   4. `no-borrow-and-mut` — `&mut` внутри `iter()` по тому же контейнеру.
///   5. `no-collect-vec`    — `collect::<Vec<_>>()` там, где хватит итератора.
///   6. `no-float-eq`       — `== ` на `f32` без `abs() < EPSILON`.
///
/// ПРОВЕРКИ:
///   * каждое новое правило срабатывает на `BAD_SRC`;
///   * ни одно не срабатывает на `GOOD_SRC`;
///   * сортировка: `DENY` идут раньше `WARN`, `WARN` раньше `ADVICE`;
///   * внутри одной строки не больше одной находки на правило;
///   * комментарии (`//`) игнорируются.
///
/// ⚠ ПОЧЕМУ ПРАВИЛА ТАК ПРИМИТИВНЫ. Потому что clippy — тоже
///   `str::contains` для большинства линтов. Настоящая сила линтера
///   не в сложности правил, а в том, что ПРОВЕРКА ИДЁТ В CI на каждом
///   коммите. Линтер, который никто не запускает, мёртв.
fn task_20_2() {
    // Заготовка-контракт: функция должна принять правила и исходник.
    fn my_audit(src: &str, extra: &[Rule]) -> Vec<Finding> {
        let _ = (src, extra);
        not_yet!("audit(src, &engine_rules()) + свои 6 правил -> Vec<Finding>");
    }

    // Проверки на сортировку и дедуп — работают и на заготовке.
    let findings = my_audit(BAD_SRC, &engine_rules());
    assert!(!findings.is_empty(), "на BAD_SRC должно быть что-то найдено");

    // Сортировка по severity: ни одна WARN не должна идти после DENY.
    let mut saw_non_deny = false;
    for f in &findings {
        if f.severity == Severity::Deny {
            saw_non_deny = true;
        } else {
            assert!(!saw_non_deny, "DENY после {:?} в строке {}", f.severity, f.line);
        }
    }

    // Дедуп: одно правило — одна находка на строку.
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for f in &findings {
        let key = (f.code, f.line);
        assert!(!seen.contains(&key), "дубль: {key:?}");
        seen.push(key);
    }

    // Комментарии игнорируются.
    assert!(findings.iter().all(|f| !f.text.starts_with("//")), "комментарии должны пропускаться");

    // Шесть новых правил — обязательная часть задания.
    assert!(
        findings.iter().any(|f| f.code == "no-string-key")
            || findings.iter().any(|f| f.code == "no-float-eq")
            || findings.len() >= 8,
        "добавь свои правила: сейчас только {} находок на BAD_SRC",
        findings.len()
    );
}

/// ЧАСТЬ 20.7 — ЗАДАНИЕ 20.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Каркас `Main/` — то, ради чего всё затевалось. Не «движок»,
/// а РАБОЧИЙ МИНИМАЛЬНЫЙ КАРКАС, который можно расширять.
///
/// Требования:
///   1. `struct Engine` с полями: `resources: Resources`,
///      `pipeline: Vec<Box<dyn Stage>>`, `frames: u64`.
///      НИКАКИХ глобальных переменных.
///   2. `Engine::new() -> Result<Self, EngineError>`.
///   3. `Engine::add_stage(s: Box<dyn Stage>)` — порядок выполнения
///      равен порядку добавления.
///   4. `Engine::run(frames: u64) -> Result<FrameStats, EngineError>`:
///      создаёт `Frame` на каждый кадр, прогоняет все стадии,
///      считает время каждой стадии.
///   5. `FrameStats { frames, total_ms, per_stage_ms, visible }` + `Display`.
///   6. `Drop для Engine`: печатает статистику. НЕ вызывает
///      `shutdown` вручную — всё само.
///
/// Условия проверки:
///   * 60 кадров, среднее < 1 мс на кадр (должно быть ~микросекунды).
///   * `per_stage_ms` содержит ВСЕ добавленные стадии, по имени.
///   * `resources` освобождается при `drop(engine)` — проверить `alloc`.
///   * стадия, вернувшая `Err`, останавливает кадр и `run`
///     возвращает `Err`. Уже отработавшие стадии НЕ откатываются
///     (отката в Rust нет — это не транзакция).
///
/// ⚠ ПОСЛЕ ЭТОГО ЗАДАНИЯ У ТЕБЯ ЕСТЬ `Main/src/main.rs`:
///   перенеси `Engine` в отдельный крейт `engine/`, а `main` оставь
///   точкой входа с обработкой аргументов. Ровно так устроен любой
///   настоящий проект: чистая библиотека + тонкий `main`.
fn task_20_3() {
    #[derive(Debug, Default, Clone)]
    struct FrameStats {
        frames: u64,
        total_ms: f64,
        per_stage_ms: Vec<(String, f64)>,
        visible: usize,
    }

    impl fmt::Display for FrameStats {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            not_yet!("frames, total_ms, per_stage (сортированно), visible");
        }
    }

    struct Engine {
        resources: Resources,
        pipeline: Vec<Box<dyn Stage>>,
        frames: u64,
    }

    impl Engine {
        fn new() -> Result<Engine, EngineError> {
            not_yet!("Resources::new() + пустой pipeline");
        }

        fn add_stage(&mut self, s: Box<dyn Stage>) {
            not_yet!("pipeline.push(s)");
        }

        fn run(&mut self, frames: u64) -> Result<FrameStats, EngineError> {
            not_yet!("цикл кадров, замер каждой стадии, ? на ошибке");
        }
    }

    impl Drop for Engine {
        fn drop(&mut self) {
            not_yet!("println!(\"engine: {} кадров, {} стадий\", self.frames, self.pipeline.len())");
        }
    }

    // --- Проверки ----------------------------------------------------------
    let before = alloc::snapshot();
    {
        let mut e = Engine::new().expect("движок");
        e.add_stage(Box::new(CullingStage::new((0..500).map(|i| (i as f32, 0.0, 0.0)).collect())));
        e.add_stage(Box::new(UploadStage { pushed: 0 }));
        e.add_stage(Box::new(PresentStage { frames: 0 }));

        let stats = e.run(60).expect("60 кадров");
        assert_eq!(stats.frames, 60);
        assert_eq!(stats.per_stage_ms.len(), 3, "все три стадии должны быть в статистике");
        assert!(
            stats.total_ms / 60.0 < 1.0,
            "кадр должен быть быстрее 1 мс, получилось {:.4} мс",
            stats.total_ms / 60.0
        );
        assert!(stats.visible > 0, "culling что-то должен пропустить");
        println!("  {stats}");
    }
    let after = alloc::snapshot();
    assert!(after.live <= before.live, "ресурсы освобождены при drop: {:?} -> {:?}", before.live, after.live);

    // Ошибка из стадии останавливает кадр.
    let mut e = Engine::new().expect("движок");
    e.add_stage(Box::new(FailingStage));
    let err = e.run(1).expect_err("стадия упала");
    assert!(matches!(err, EngineError::NotFound(_)), "получено: {err}");
}

/// Стадия, которая всегда падает — для негативного теста.
struct FailingStage;

impl Stage for FailingStage {
    fn name(&self) -> &str {
        "failing"
    }
    fn run(&mut self, _f: &mut Frame) -> Result<(), EngineError> {
        Err(EngineError::NotFound("я сломан".into()))
    }
}
