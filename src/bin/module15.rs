//! ============================================================================
//! МОДУЛЬ 15 — КОНКУРЕНТНОСТЬ
//! ============================================================================
//!
//! ЧАСТЬ 15.1 — ТЕОРИЯ: потоки и каналы
//! ─────────────────────────────────────────────────────────────────────────────
//! В C++ для потока нужен `std::thread` + мьютекс + condvar + гонка в
//! `std::atomic` по умолчанию (никакой). В Rust компилятор ЗАПРЕЩАЕТ
//! передать не-Send тип в другой поток. Это меняет характер кода:
//! ты не «стараешься не гонять», ты физически не можешь.
//!
//! ТРИ СПОСОБА ОБМЕНА ДАННЫМИ, от дешёвого к дорогому:
//!
//!   1. Атомики          — для счётчиков и флагов. ~1-20 нс.
//!   2. `Arc<Mutex<T>>`  — короткая критическая секция. ~20-50 нс.
//!   3. `mpsc` канал     — передача владения между потоками. Блокирует.
//!   4. Копирование      — если данные мелкие, `Send + Copy` дешевле всего.
//!
//! ⚠ ПРАВИЛО: НЕ ДЕЛАЙ copy ради безопасности. Сначала спроси, нужен
//!   ли вообще доступ из двух потоков. Горячий цикл в одном потоке +
//!   обмен результатами раз в кадр — почти всегда быстрее, чем
//!   параллелизм с блокировкой.
//!
//! `std::thread::scope` — сценарий, который забывают:
//!   обычный `spawn` требует `'static` (данные не могут ссылаться на стек),
//!   поэтому приходится `move` + `Arc` + клонирование. `scope` снимает
//!   это требование: потоки живут только внутри блока scope и могут
//!   заимствовать обычные переменные. Никаких `Arc`, никакого `clone`.
//!   Именно поэтому `split_at_mut` в модуле 2 становится реально полезным.
//!
//! КАНАЛЫ `std::sync::mpsc`:
//!   * `channel()` — неограниченный. `Sender` клонируется (много
//!     производителей), `Receiver` один.
//!   * `sync_channel(n)` — ограниченный. Это ЕДИНСТВЕННЫЙ способ
//!     сделать нормальное многопоточное производство-потребление без
//!     busy-wait: отправитель блокируется, когда очередь полна.
//!   * `recv()` блокирует, `try_recv()` — нет, `recv_timeout` — с таймаутом.
//!   * ⚠ `recv` возвращает `Err`, когда ВСЕ отправители умерли. Это не
//!     ошибка, а штатное завершение цикла: `while let Ok(x) = rx.recv()`.
//!
//! `crossbeam-channel` быстрее std-варианта и умеет `select!` (ждать
//! несколько каналов). В реальном проекте бери его.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

use std::sync::mpsc;

fn main() {
    harness::module(15, "Конкурентность");

    part_15_1(); // потоки, scope, каналы
    part_15_2(); // блокировки, атомики, порядок памяти
    part_15_3(); // Send/Sync границы, thread_local, распараллеливание

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("15.1  Параллельная генерация меша через scope + split_at_mut", task_15_1);
    r.task("15.2  Пул загрузки: Mutex + OnceLock + condvar вместо busy-wait", task_15_2);
    r.task("15.3  Пула воркеров с разделением работы и reduce", task_15_3);
    r.summary(15);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 15.1 — ПРИМЕР
// ─────────────────────────────────────────────────────────────────────────────
fn part_15_1() {
    harness::part("15.1", "ПРИМЕР: потоки, scope, каналы");

    // --- Базовый поток -------------------------------------------------------
    let handle = std::thread::spawn(|| 1 + 1);
    println!("  обычный поток вернул: {}", handle.join().expect("поток не паниковал"));
    // ⚠ spawn требует 'static. Поэтому:
    //   let v = vec![1,2,3];
    //   thread::spawn(|| println!("{v:?}"));         // ⛔ E0373
    //   thread::spawn(move || println!("{v:?}"));      // ok, v перемещён

    // --- scope: заимствование без Arc и clone -------------------------------
    let data = vec![1u32, 2, 3, 4, 5, 6, 7, 8];
    let half = data.len() / 2;
    let (left, right) = data.split_at(half); // из модуля 2!

    // Из модуля 2 мы знаем, что эти ДВА слоя не пересекаются,
    // поэтому компилятор разрешает два изменяющих займа.
    let (a, b) = std::thread::scope(|s| {
        let h1 = s.spawn(|| left.iter().filter(|x| **x % 2 == 1).sum::<u32>());
        let h2 = s.spawn(|| right.iter().filter(|x| **x % 2 == 1).sum::<u32>());
        (h1.join().expect("поток"), h2.join().expect("поток"))
    });
    println!("  scope: нечётных в двух половинах = {a} и {b} (итого {})", a + b);
    println!("    ноль Arc, ноль clone, две ссылки на общий Vec");

    // --- Каналы: один производитель -----------------------------------------
    let (tx, rx) = mpsc::channel();
    for i in 0..4u32 {
        tx.send(i * i).expect("приёмник жив");
    }
    drop(tx); // ⚠ без этого recv() зависнет: канал закрыт, когда умерли ВСЕ отправители
    let received: Vec<u32> = rx.iter().collect();
    println!("  channel: квадраты = {received:?}");
    println!("    ⚠ drop(tx) обязателен: иначе цикл примет ещё элементов и зависнет");

    // --- Канал: несколько производителей -------------------------------------
    let (tx, rx) = mpsc::channel();
    for t in 0..3u32 {
        let tx = tx.clone(); // клон отправителя — дёшево
        std::thread::spawn(move || {
            for i in 0..2u32 {
                tx.send(t * 10 + i).expect("приёмник жив");
            }
            // tx умирает здесь — счётчик отправителей уменьшается
        });
    }
    drop(tx); // наш экземпляр тоже отпускаем
    let mut got: Vec<u32> = rx.iter().collect();
    got.sort_unstable();
    println!("  3 потока-производителя → {got:?} (порядок НЕ гарантирован)");

    // --- Ограниченный канал: честное backpressure ---------------------------
    let (tx, rx) = mpsc::sync_channel(2);
    let producer = std::thread::spawn(move || {
        for i in 0..6u32 {
            // Блокируется, пока очередь не освободится. Это и есть
            // backpressure: медленный потребитель тормозит производителя,
            // вместо того чтобы съесть всю память.
            tx.send(i).expect("приёмник жив");
        }
    });
    let got: Vec<u32> = rx.iter().collect();
    producer.join().expect("производитель");
    println!("  sync_channel(2): получено {got:?} без переполнения");

    // --- try_recv и таймаут --------------------------------------------------
    let (tx, rx) = mpsc::channel();
    tx.send(7u32).expect("приёмник жив");
    println!("  try_recv = {:?}", rx.try_recv());
    println!("  try_recv (пусто) = {:?}", rx.try_recv().is_err());
    let timeout = rx.recv_timeout(std::time::Duration::from_millis(50));
    println!("  recv_timeout(50ms) = {:?}", timeout.map_err(|e| format!("{e:?}")));
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 15.2 — ТЕОРИЯ: блокировки, атомики, порядок памяти
// ─────────────────────────────────────────────────────────────────────────────
//
// ORDERING В АТОМИКАХ — САМАЯ ТОНКАЯ ТЕМА В RUST, и вот почему.
//
// Компилятор переставляет операции. Процессор переставляет операции
// (из-за буферов записи). Если поток A пишет данные, а поток B читает
// их без синхронизации, B может увидеть «новый флаг, старые данные».
//
// Четыре уровня, от дешёвого к дорогому:
//
//   Relaxed  — атомарность операции есть, но НИКАКОГО порядка с другими
//              операциями. Для счётчика «сколько раз посчитали» —
//              идеально. Для флагов — нет.
//   Acquire  — ЗАПРЕЩАЕТ перестановку ПОСЛЕ себя. «Увидев этот флаг,
//              ты гарантированно увидишь и всё, что было записано до
//              него». Это чтение.
//   Release  — запрещает перестановку ДО себя. Это запись.
//              Release + Acquire образуют пару «записал → прочитал».
//   SeqCst   — плюс полный глобальный порядок между ВСЕМИ атомиками.
//              Проще рассуждать, дороже по скорости. Почти всегда
//              достаточно Release/Acquire.
//
// ПРАВИЛО ДЛЯ ФЛАГОВ: пиши через `Release`, читай через `Acquire`.
// Для счётчиков: `Relaxed` хватает.
//
// УСЛУЖНЫЕ ТИПЫ:
//   `Condvar`  — «ждать, пока условие станет true». ⚠ всегда с циклом:
//                 `while !cond { cv.wait(lock) }`, потому что между
//                 проверкой и засыпанием условие может измениться.
//                 Это называется «lost wakeup», и в C++ он у вас тоже
//                 есть, только в `std::condition_variable`.
//   `Barrier`  — все потоки ждут друг друга. Для «фаз» в задаче.
//   `OnceLock` — «вычисли один раз, лениво, без гонок».
//   `park` / `unpark` — ручной condvar без блокировки, эффективнее.
//
// DEADLOCK. С `Mutex` в Rust его легко получить: два замка в разном
// порядке в двух ветках кода. Правило — ВСЕГДА брать замки в
// ОДИНАКОВОМ порядке, и по возможности не держать два сразу.

fn part_15_2() {
    harness::part("15.2", "ПРИМЕР: RwLock, атомики, порядок памяти, condvar");

    use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
    use std::sync::{Arc, Condvar, Mutex, RwLock};

    // --- RwLock --------------------------------------------------------------
    let data = Arc::new(RwLock::new(vec![1u32, 2, 3]));
    {
        let mut w = data.write().expect("не отравлен");
        w.push(4);
    }
    let readers: Vec<usize> = (0..4)
        .map(|_| data.read().expect("не отравлен").len())
        .collect();
    println!("  RwLock: записали, 4 читателя видели длину {readers:?}");

    // --- Атомика: счётчик ----------------------------------------------------
    let hits = Arc::new(AtomicU32::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let mut hs = Vec::new();
    for _ in 0..4 {
        let h = Arc::clone(&hits);
        let d = Arc::clone(&done);
        hs.push(std::thread::spawn(move || {
            for _ in 0..10_000 {
                h.fetch_add(1, Ordering::Relaxed);
            }
            // «Один поток закончил» — это ФЛАГ, ему нужен порядок:
            d.store(true, Ordering::Release);
        }));
    }
    for h in hs {
        h.join().expect("поток");
    }
    // Acquire «видит» всё, что было записано до Release в другом потоке.
    while !done.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    println!("  AtomicU32: 4*10000 = {} fetch_add(Relaxed) — данные не защищены, но и не портятся", hits.load(Ordering::Relaxed));

    // --- Стоимость: атомика против мьютекса -----------------------------------
    let b_atomic = curriculum::harness::bench("AtomicU32 fetch_add x1M", 200, |_| {
        hits.fetch_add(1, Ordering::Relaxed);
    });
    let m = Arc::new(Mutex::new(0u32));
    let b_mutex = curriculum::harness::bench("Mutex<u32> lock x1M", 200, |_| {
        let mut g = m.lock().expect("не отравлен");
        *g += 1;
    });
    curriculum::harness::show(&b_atomic);
    curriculum::harness::show(&b_mutex);
    println!("  → для одного счётчика атомика в разы дешевле мьютекса");

    // --- Condvar: producer/consumer без busy-wait ----------------------------
    // Arc<(Mutex<Vec<u32>>, Condvar)> — «очередь + звонок». Так пишут,
    // когда не хочется заводить отдельную структуру.
    let queue = Arc::new((Mutex::new(Vec::<u32>::new()), Condvar::new()));
    let mut handles = Vec::new();
    for t in 0..3u32 {
        let q = Arc::clone(&queue);
        handles.push(std::thread::spawn(move || {
            for i in 0..4u32 {
                // ⚠ ЦИКЛ, а не `if`: между проверкой и wait условие
                // может измениться — это «lost wakeup».
                let mut g = q.0.lock().expect("не отравлен");
                while g.len() >= 8 {
                    g = q.1.wait(g).expect("не отравлен");
                }
                g.push(t * 10 + i);
                q.1.notify_one();
            }
        }));
    }
    for h in handles {
        h.join().expect("поток");
    }
    let g = queue.0.lock().expect("не отравлен");
    println!("  Condvar: {} задач разложено без единого busy-wait", g.len());
    drop(g);

    // --- parking_lot в реальном проекте --------------------------------------
    println!("\n  parking_lot (в реальном проекте):");
    println!("    * RwLock без poisoning");
    println!("    * «честная» блокировка: читатель не заstarживает писателя");
    println!("    * в 2-10 раз меньше накладных на uncontended contended пути");
    println!("    * parking_lot::Condvar без spurious wakeup");
    println!("    ⚠ Минус: PoisonError исчезает, и паника в критической секции");
    println!("      оставит данные в неизвестном состоянии молча.");
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 15.3 — ТЕОРИЯ: границы Send/Sync, thread_local, параллелизм
// ─────────────────────────────────────────────────────────────────────────────
//
// `Send` = «можно передать в другой поток» (владение переезжает).
// `Sync` = «можно дать `&` в другой поток» (значение остаётся тут).
//
//   Rc<T>       — !Send, !Sync  (счётчик не атомарен)
//   Cell<T>     — Send если T: Send, но !Sync (через & можно менять)
//   RefCell<T>  — Send если T: Send, но !Sync
//   Mutex<T>    — Send и Sync если T: Send
//   *const T    — никогда (сырой указатель не владеет памятью)
//   PhantomData — влияет на вывод, см. модуль 11
//
// ОТСЮДА ПРАВИЛО ДЛЯ FFI-ОБЁРТОК (модуль 18): RAII-обёртка над
// `VkDevice` содержит сырой указатель, поэтому `!Send` по умолчанию.
// Это БЕЗОПАСНО. Если хочешь передать устройство в поток рендера —
// нужен явный `unsafe impl Send` с обоснованием, что драйвер это
// допускает (для Vulkan 1.x — да, для отладочных слоёв — осторожно).
//
// `thread_local!` — значение на каждый поток. Инструмент для кешей,
// которые нельзя синхронизировать без потери смысла:
//   * счётчик обращений к статистике (для профилировщика);
//   * временный буфер парсера (переиспользовать между вызовами);
//   * RNG в каждом потоке со своим seed.
//
// WORK-STEALING (как в `rayon`). Идея: есть общая очередь задач и
// локальная очередь у каждого потока. Поток сначала берёт из своей
// очереди (дёшево, без блокировки), а когда она пуста — крадёт у
// соседа (дороже, но сбалансированно). Это даёт лучшую локальность
// кэша, чем одна общая очередь на мьютексе.
//
// КОГДА ХВАТАЕТ ОДНОГО ПОТОКА: если память — узкое место (а не CPU),
// параллелизм только мешает (кеш вытесняется между ядрами). Мешает
// значит: два ядра пишут в один кэш-линии, и это медленнее, чем
// один писатель. Модуль 13 показал это в 2.1 раза.

fn part_15_3() {
    harness::part("15.3", "ПРИМЕР: Send/Sync, thread_local, параллель map");

    // --- Проверка границ Send/Sync компилятором -----------------------------
    assert_send::<Vec<u8>>();
    assert_send::<std::sync::Arc<std::sync::Mutex<u32>>>();
    assert_send::<std::sync::Arc<std::sync::RwLock<u32>>>();
    // assert_send::<std::rc::Rc<u32>>();        // ⛔ Rc не Send
    // assert_sync::<std::cell::Cell<u32>>();    // ⛔ Cell не Sync
    println!("  Vec, Arc<Mutex>, Arc<RwLock> — Send + Sync");
    println!("  ⛔ Rc<T> — !Send (счётчик не атомарен)");
    println!("  ⛔ Cell<T> — !Sync (через & можно получить изменение)");

    // --- thread_local -------------------------------------------------------
    std::thread_local! {
        static PARSER_BUF: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut hs = Vec::new();
    for _ in 0..4 {
        let c = std::sync::Arc::clone(&counter);
        hs.push(std::thread::spawn(move || {
            for _ in 0..100 {
                PARSER_BUF.with(|b| {
                    let mut b = b.borrow_mut();
                    b.clear();
                    b.extend_from_slice(b"0123456789");
                    assert_eq!(b.len(), 10);
                });
                c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }));
    }
    for h in hs {
        h.join().expect("поток");
    }
    println!("  thread_local: 4 потока × 100 раз дергают свой Vec, аллокаций после прогрева нет");
    println!("    главный поток тоже имеет свой: len = {}", PARSER_BUF.with(|b| b.borrow().len()));
    println!("    счётчик: {}", counter.load(std::sync::atomic::Ordering::Relaxed));

    // --- Параллельный map: два подхода --------------------------------------
    let data: Vec<u32> = (0..200_000u32).map(|i| i % 1000).collect();

    let sequential: u64 = data.iter().map(|x| *x as u64 * *x as u64).sum();

    // Подход 1: скраб по индексам (то, что делает rayon)
    let t = std::time::Instant::now();
    let threads = 4;
    let chunk = data.len().div_ceil(threads);
    let parallel: u64 = std::thread::scope(|s| {
        let handles: Vec<_> = data.chunks(chunk).map(|part| s.spawn(|| part.iter().map(|x| *x as u64 * *x as u64).sum::<u64>())).collect();
        handles.into_iter().map(|h| h.join().expect("поток")).sum()
    });
    let par_time = t.elapsed();

    println!("  сумма квадратов: seq={sequential} par={parallel} (совпало: {})", sequential == parallel);
    println!("  параллельно: {par_time:?} на {threads} потоках");
    println!("    ⚠ Сравни с последовательным замером через harness::bench — на этой");
    println!("      задаче разница может быть нулевой или отрицательной: узкое место —");
    println!("      ПАМЯТЬ, а не CPU. Это нормально и очень частая ситуация.");

    // --- Проверяем, что split_at_mut даёт два независимых &mut ----------------
    let mut buf = vec![0u8; 8];
    {
        let (l, r) = buf.split_at_mut(4);
        let hl = std::thread::scope(|s| {
            let h = s.spawn(move || {
                l[0] = 1;
                l.len()
            });
            h.join().expect("поток")
        });
        assert_eq!(hl, 4);
        r[0] = 2;
    }
    println!("  split_at_mut + scope: buf[0]={} buf[4]={}", buf[0], buf[4]);
}

struct RwLockAlias<T>(std::sync::RwLock<T>);

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

use std::sync::atomic::AtomicU32 as AtomicU32Alias;

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 15.4 — ЗАДАНИЕ 15.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Параллельная обработка буфера. Ключевой навык — `thread::scope`
/// плюс `split_at_mut`: оба появились в модулях 2 и 15, здесь они
/// встречаются вместе.
///
/// ```ignore
/// fn normalize_chunks(verts: &mut [f32], chunk: usize) -> usize;
/// fn process(mut verts: Vec<f32>, threads: usize) -> (Vec<f32>, usize);
/// ```
///
/// Требования:
///   * `normalize_chunks` делит срез на куски длиной `chunk`,
///     нормализует каждый ОТДЕЛЬНО и возвращает число обработанных.
///   * Куски раздаются потокам через `thread::scope` (никаких `Arc`!).
///   * ⚠ ГЛАВНАЯ СЛОЖНОСТЬ: как раздать ПО ССЫЛКЕ на каждый кусок,
///     если кусков больше, чем потоков? Ответ — границы индексов и
///     `split_at_mut` в цикле, либо собирать заёмы в Vec *до* scope.
///     ⚠ Обрати внимание: нельзя держать `&mut` на кусок и одновременно
///     создавать следующий — `split_at_mut` решает и это.
///   * `process` возвращает (новый буфер, число потоков реально
///     задействованных). Сравни результат с последовательным.
//   * `chunk == 0` → вернуть `usize::MAX` (защита от деления на ноль)
///     или обработать как один кусок. Выбери и напиши почему.
fn task_15_1() {
    use std::thread;

    fn normalize_chunk(chunk: &mut [f32]) -> usize {
        let mut n = 0;
        for t in chunk.chunks_exact_mut(3) {
            let len = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
            if len > f32::EPSILON {
                t[0] /= len;
                t[1] /= len;
                t[2] /= len;
                n += 1;
            }
        }
        n
    }

    fn process(verts: Vec<f32>, threads: usize) -> (Vec<f32>, usize) {
        // ⚠ `verts: &mut Vec` нельзя раздать потокам, пока сам же
        //   владеешь им. Внутри функции Vec — локальная переменная,
        //   и scope разрешает заимствовать её на время scope.
        not_yet!("split_at_mut по границам кусков + thread::scope + join");
    }

    fn normalize_chunks(verts: &mut [f32], chunk: usize) -> usize {
        not_yet!("верни сумму обработанных кусков; chunk == 0 -> usize::MAX");
    }

    // sequential reference
    let mut seq: Vec<f32> = (0..30).map(|i| (i % 7) as f32 + 0.5).collect();
    let expected = normalize_chunks(&mut seq, 9);
    assert!(expected > 0);

    let (out, used) = process(seq.clone(), 3);
    assert_eq!(used, 3);
    assert_eq!(out, seq, "параллельный результат == последовательному");

    assert_eq!(normalize_chunks(&mut seq.clone(), 0), usize::MAX);
}

/// ЧАСТЬ 15.5 — ЗАДАНИЕ 15.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Пул потоков загрузки ресурсов. Ключевая идея: НИКАКОГО busy-wait.
/// В C++ «ждать, пока освободится» обычно означает `while (busy) {}`,
/// и это съедает ядро. В Rust это `Condvar` или канал.
///
/// ```ignore
/// struct Loader { jobs: usize, results: usize }
///
/// fn parallel_map<T: Send, R: Send>(items: Vec<T>, f: impl Fn(T) -> R + Send + Sync)
///     -> Vec<R>;
///
/// fn fan_out(tasks: Vec<(u32, u32)>, threads: usize) -> Vec<u32>;
/// ```
///
/// Требования:
///   * `parallel_map` через `thread::scope` + `chunks`, результат
///     в исходном порядке (индексы сохраняются).
///   * `fan_out` — иллюстрирует «поток делает и ждёт»: используй
///     `Condvar` c циклом, без `spin_loop`. Прокомментируй, где именно
///     выигрывает condvar перед `yield_now`.
///   * Бонус: `fn busy_wait(tasks: Vec<u32>) -> Vec<u32>` — реализуй
///     НАМЕРЕННО плохо, через `std::thread::yield_now()`, и сравни
///     замеры. Объясни результат.
fn task_15_2() {
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;

    fn parallel_map<T: Send, R: Send>(
        items: Vec<T>,
        f: impl Fn(T) -> R + Send + Sync,
    ) -> Vec<R> {
        not_yet!("scope + chunks + собрать в Vec с правильным порядком");
    }

    fn fan_out(tasks: Vec<(u32, u32)>, threads: usize) -> Vec<u32> {
        // Задача: (вход, задержка_мс) -> выход = вход * задержка.
        // Потоки должны брать задачи из общей очереди, а не ждать своей.
        not_yet!("Mutex<VecDeque<(u32,u32)>> + Condvar + счётчик выполненных");
    }

    fn busy_wait(tasks: Vec<u32>) -> Vec<u32> {
        // ⚠ НАМЕРЕННО ПЛОХОЙ КОД — так пишут новички. Сравни с fan_out.
        let done = Arc::new(AtomicU32Wrap::new(0));
        let mut hs = Vec::new();
        for t in tasks {
            let d = Arc::clone(&done);
            hs.push(thread::spawn(move || {
                while d.load(std::sync::atomic::Ordering::Relaxed) < 3 {
                    std::thread::yield_now();
                }
                d.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                t * 2
            }));
        }
        let mut out = Vec::new();
        for h in hs {
            out.push(h.join().expect("поток"));
        }
        out.sort_unstable();
        out
    }

    struct AtomicU32Wrap(std::sync::atomic::AtomicU32);
    impl AtomicU32Wrap {
        fn new(v: u32) -> Self {
            Self(std::sync::atomic::AtomicU32::new(v))
        }
        fn load(&self, o: std::sync::atomic::Ordering) -> u32 {
            self.0.load(o)
        }
        fn fetch_add(&self, v: u32, o: std::sync::atomic::Ordering) -> u32 {
            self.0.fetch_add(v, o)
        }
    }

    // --- parallel_map -------------------------------------------------------
    let out = parallel_map((1..=1_000u32).collect::<Vec<_>>(), |x| x * x);
    assert_eq!(out.len(), 1000);
    assert_eq!(out[0], 1);
    assert_eq!(out[999], 999 * 999);
    // ПОРЯДОК важен: результат i соответствует items[i]

    // --- fan_out ------------------------------------------------------------
    let tasks: Vec<(u32, u32)> = (0..12u32).map(|i| (i, (i % 3) + 1)).collect();
    let mut got = fan_out(tasks.clone(), 4);
    got.sort_unstable();
    let mut want: Vec<u32> = tasks.iter().map(|(a, b)| a * b).collect();
    want.sort_unstable();
    assert_eq!(got, want);

    // --- Замер: плохой код против хорошего ----------------------------------
    let small: Vec<u32> = (0..6u32).collect();
    let b_busy = curriculum::harness::bench("busy_wait (yield_now)", 5, |_| busy_wait(small.clone()));
    let b_fan = curriculum::harness::bench("fan_out (condvar)", 5, |_| fan_out(vec![(1, 1); 6], 4));
    curriculum::harness::show(&b_busy);
    curriculum::harness::show(&b_fan);
    println!("  ⚠ busy_wait ждёт, пока счётчик дойдёт до 3 — он НИКОГДА не дойдёт");
    println!("    в одиночку, поэтому здесь только сравнение затрат на ожидание.");
    let _ = (Arc::new(()), Mutex::new(0), Condvar::new());
}

/// ЧАСТЬ 15.6 — ЗАДАНИЕ 15.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Простой пул воркеров с общей очередью — замена `rayon` для понимания
/// основ. Пул создаётся один раз и переиспользуется (создавать поток
/// на задачу — дорого: ~50 мкс на поток в Windows).
///
/// ```ignore
/// struct Pool { workers: Vec<std::thread::JoinHandle<()>>, jobs: Arc<JobQueue> }
/// struct JobQueue { tx: Mutex<Vec<Box<dyn FnOnce() + Send>>>, cv: Condvar }
///
/// impl Pool {
///     fn new(size: usize) -> Self;
///     fn submit(&self, job: Box<dyn FnOnce() + Send>);
///     fn finish(self);   // дождаться всех
/// }
/// ```
///
/// Требования:
///   * Воркер: цикл `while let Some(job) = pop() { job() }`, и выход
///     по `None` (очередь пуста И пул закрыт).
///   * `submit` кладёт замыкание в очередь и делает `notify_one`.
///   * ⚠ `Box<dyn FnOnce() + Send>` — именно `FnOnce`, потому что
///     замыкание может владеть ресурсами, которые нельзя клонировать.
///   * `finish(self)` кладёт N × `None` и делает `notify_all`, затем join.
///   * Проверь, что задачи выполнились ВСЕ, даже если их больше, чем
///     воркеров, и что порядок выполнения не гарантирован.
///   * Замерь: стоимость `submit` против `thread::spawn` на задачу.
fn task_15_3() {
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;

    struct JobQueue {
        jobs: Mutex<Option<Vec<Box<dyn FnOnce() + Send>>>>,
        cv: Condvar,
    }

    struct Pool {
        workers: Vec<thread::JoinHandle<()>>,
        q: Arc<JobQueue>,
    }

    impl Pool {
        fn new(size: usize) -> Self {
            not_yet!("создай очередь и size воркеров");
        }

        fn submit(&self, job: Box<dyn FnOnce() + Send>) {
            not_yet!("запри очередь, push, notify_one");
        }

        fn finish(self) {
            not_yet!("запри, запиши None как «закрыто», notify_all, join всех");
        }
    }

    let counter = Arc::new(Mutex::new(0u32));
    let pool = Pool::new(4);
    for i in 0..100u32 {
        let c = std::sync::Arc::clone(&counter);
        pool.submit(Box::new(move || {
            *c.lock().expect("не отравлен") += i % 3;
        }));
    }
    pool.finish();
    let total = *counter.lock().expect("не отравлен");
    let expected: u32 = (0..100u32).map(|i| i % 3).sum();
    assert_eq!(total, expected, "все задачи выполнены");

    println!("  пул из 4 воркеров выполнил 100 задач, сумма {total}");
}
