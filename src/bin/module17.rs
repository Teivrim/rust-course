//! ============================================================================
//! МОДУЛЬ 17 — МОДЕЛЬ ДАННЫХ, СЕРИАЛИЗАЦИЯ, ECS
//! ============================================================================
//!
//! ЧАСТЬ 17.1 — ТЕОРИЯ: Serialize / Deserialize своими руками
//! ─────────────────────────────────────────────────────────────────────────────
//! В реальном проекте это `serde` + `#[derive(Serialize, Deserialize)]`.
//! Одна строка в Cargo.toml, и все структуры получают `to_json()`.
//!
//! ЗДЕСЬ МЫ ПИШЕМ ЭТО САМИ, потому что понимание обязательно:
//! когда сохранение сцены «работает, но неправильно», чинить придётся
//! вручную. И вот главные решения, которые принимает serde:
//!
//!   1. ФОРМАТ. Обычно JSON (текст, читаемый, но медленный и неоднозначный
//!      для float) или бинарный (быстрый, но ломается при изменении
//!      структуры). Мы сделаем оба и сравним.
//!   2. ЧТО СЕРИАЛИЗУЕТСЯ. Только поля, помеченные явно, или все
//!      (derive). Пропущенное поле при чтении → дефолт или ошибка.
//!   3. НЕИЗВЕСТНЫЕ ПОЛЯ. Хороший формат их игнорирует (forward compat).
//!      Плохой — падает. Для движка это критично: старый сейв должен
//!      открываться новым кодом.
//!   4. ВЕРСИЯ. В файл кладётся версия схемы. Без неё любое изменение
//!      структуры ломает все сохранения.
//!
//! ТРАКТИЧЕСКИЙ ВЫВОД ДЛЯ ДВИЖКА: сцену сохраняй в СВОЁМ бинарном
//! формате с магическим числом, версией и явным списком блоков.
//! JSON хорош для конфига и отладочных дампов, плох для ассетов.
//!
//! БИНАРНЫЙ ФОРМАТ, КОТОРЫЙ СТОИТ СДЕЛАТЬ (30 строк):
//!
//!   magic:  u32   "NOVE"
//!   version:u16   1
//!   flags:  u16
//!   entities: u32 + [u32; N]        (id каждой сущности)
//!   components: u16 количество типов
//!     для каждого: type_id: u16, count: u32, data: [u8; count*size]
//!
//! Так добавление нового типа компонента не ломает старые файлы:
//!   «неизвестный type_id» → пропустить, если флаг «строгий» не выставлен.
//!   Именно так работает ECS в реальных движках (Unity, Unreal).

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

fn main() {
    harness::module(17, "Модель данных, сериализация, ECS");

    part_17_1(); // свой Serialize/Deserialize на голом std
    part_17_2(); // ECS: sparse set, запросы, структурные изменения
    part_17_3(); // AssetManager: кеш, дедуп, type erasure

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("17.1  Бинарный формат сцены: версия, магия, round-trip", task_17_1);
    r.task("17.2  Мини-ECS: sparse set, запросы, command buffer", task_17_2);
    r.task("17.3  AssetManager с кешем, дефолтами и дедупликацией", task_17_3);
    r.summary(17);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 17.1 — ПРИМЕР: свой трейт сериализации
// ─────────────────────────────────────────────────────────────────────────────

/// Минимальный трейт записи. В serde это `Serializer` — обобщённый
/// над форматом, и поэтому он такой сложный. Здесь формат один,
/// и трейт простой: «умей записать себя в буфер».
trait Encode {
    /// Пишет поля в `out` и возвращает `Result<(), EncodeError>`.
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError>;
}

/// Минимальный трейт чтения.
trait Decode: Sized {
    /// ⚠ Читает поле из `buf[pos..]`, САМ продвигая `pos`.
    ///   Возврат `Option<Self>` вместо паники — обязателен: битый файл
    ///   это норма, а не исключение.
    fn decode(buf: &[u8], pos: &mut usize) -> Option<Self>;
}

#[derive(Debug, PartialEq, Clone)]
struct EncodeError(String);

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "encode: {}", self.0)
    }
}

impl Encode for u32 {
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.to_le_bytes());
        Ok(())
    }
}

impl Decode for u32 {
    fn decode(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let s = buf.get(*pos..*pos + 4)?;
        *pos += 4;
        Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
}

impl Encode for String {
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let n = u32::try_from(self.len()).map_err(|_| EncodeError("строка длиннее 4 ГБ".into()))?;
        n.encode(out)?;
        out.extend_from_slice(self.as_bytes());
        Ok(())
    }
}

impl Decode for String {
    fn decode(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let n = u32::decode(buf, pos)? as usize;
        let s = buf.get(*pos..*pos + n)?;
        *pos += n;
        String::from_utf8(s.to_vec()).ok()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Transform {
    x: f32,
    y: f32,
    z: f32,
}

impl Encode for Transform {
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        // ⚠ Порядок байтов ЗАФИКСИРОВАН в коде. Это и есть контракт формата:
        //   сменишь на be — старые файлы перестанут читаться.
        for v in [self.x, self.y, self.z] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        Ok(())
    }
}

impl Decode for Transform {
    fn decode(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let mut v = [0f32; 3];
        for slot in &mut v {
            let s = buf.get(*pos..*pos + 4)?;
            *pos += 4;
            *slot = f32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        }
        Some(Transform { x: v[0], y: v[1], z: v[2] })
    }
}

fn part_17_1() {
    harness::part("17.1", "ПРИМЕР: свой бинарный формат");

    let scene = Scene {
        version: 1,
        name: "level_1".to_string(),
        entities: vec![
            Entity { id: 1, transform: Transform { x: 0.0, y: 0.0, z: 0.0 } },
            Entity { id: 7, transform: Transform { x: 1.5, y: -2.25, z: 100.0 } },
        ],
    };

    // --- Запись --------------------------------------------------------------
    let mut buf = Vec::new();
    scene.encode(&mut buf).expect("запись");
    println!("  сцена закодирована: {} байт", buf.len());
    println!("  первые 16 байт: {:02x?}", &buf[..16.min(buf.len())]);
    println!("    ⚠ видно: u32 length = {:02x?}, потом байты \"level_1\"", &buf[4..8]);

    // --- Чтение --------------------------------------------------------------
    let mut pos = 0usize;
    let back = Scene::decode(&buf, &mut pos).expect("чтение");
    assert_eq!(back, scene);
    println!("  round-trip: {:?} — совпало, израсходовано {pos} из {} байт", back.name, buf.len());

    // --- Устойчивость к битому файлу -----------------------------------------
    let mut truncated = buf.clone();
    truncated.truncate(truncated.len() - 3);
    let mut p = 0;
    println!("  обрезанный файл → {:?}", Scene::decode(&truncated, &mut p).map(|s| s.name));

    let garbage = vec![0xAAu8; buf.len()];
    let mut p = 0;
    match Scene::decode(&garbage, &mut p) {
        Some(s) => println!("  мусорный файл → Ok({:?}) (плохо: должно быть Err)", s.name),
        None => println!("  мусорный файл → None (корректно: без паники)"),
    }
    // ⚠ Именно это отличает нормальный парсер от «unwrap в цикле».
}

#[derive(Debug, Clone, PartialEq)]
struct Entity {
    id: u32,
    transform: Transform,
}

#[derive(Debug, Clone, PartialEq)]
struct Scene {
    version: u16,
    name: String,
    entities: Vec<Entity>,
}

impl Encode for Entity {
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.id.encode(out)?;
        self.transform.encode(out)
    }
}

impl Decode for Entity {
    fn decode(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let id = u32::decode(buf, pos)?;
        let transform = Transform::decode(buf, pos)?;
        Some(Entity { id, transform })
    }
}

impl Encode for Scene {
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        // МАГИЯ + ВЕРСИЯ — обязательная часть любого формата.
        out.extend_from_slice(b"NOVE");
        self.version.encode(out)?;
        self.name.encode(out)?;
        (self.entities.len() as u32).encode(out)?;
        for e in &self.entities {
            e.encode(out)?;
        }
        Ok(())
    }
}

impl Decode for Scene {
    fn decode(buf: &[u8], pos: &mut usize) -> Option<Self> {
        // Проверяем магию: если её нет — это не наш файл.
        if buf.get(*pos..*pos + 4)? != b"NOVE" {
            return None;
        }
        *pos += 4;
        let version = u16::decode(buf, pos)?;
        let name = String::decode(buf, pos)?;
        let count = u32::decode(buf, pos)? as usize;
        // ⚠ Проверка count ДО аллокации: иначе битый файл с count=0xFFFF_FFFF
        //   заставит нас выделить 64 ГБ и упасть с OOM.
        if count > (buf.len() - *pos) / 16 {
            return None;
        }
        let mut entities = Vec::with_capacity(count);
        for _ in 0..count {
            entities.push(Entity::decode(buf, pos)?);
        }
        Some(Scene { version, name, entities })
    }
}

impl Encode for u16 {
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.to_le_bytes());
        Ok(())
    }
}

impl Decode for u16 {
    fn decode(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let s = buf.get(*pos..*pos + 2)?;
        *pos += 2;
        Some(u16::from_le_bytes([s[0], s[1]]))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 17.2 — ТЕОРИЯ: ECS
// ─────────────────────────────────────────────────────────────────────────────
//
// СУТЬ ПОДХОДА. Не «объекты с поведением», а ДАННЫЕ отдельно от
// ЛОГИКИ:
//   Entity   — просто число (id). Ничего больше.
//   Component — данные без методов. Хранятся в массивах, разделённых
//               по типам (SoA, см. модуль 13).
//   System   — функция, которая обходит все сущности с нужными
//               компонентами и меняет их.
//
// ПОЧЕМУ ЭТО БЫСТРЕЕ (и почему так сложно):
//   * Данные лежат подряд → один проход по памяти вместо прыжков по
//     объектам (указатели разбросаны по куче).
//   * Система обходит ТОЛЬКО нужные компоненты.
//   * Никаких виртуальных вызовов и наследования.
// Цена — сложность, «низкоуровневость» и необходимость думать о
// кэше вручную. Для шутера это выигрыш, для CRUD-приложения — нет.
//
// SPARSE SET (главная структура данных ECS):
//   dense:      Vec<T>     — компоненты, идущие подряд. Обход = линейный.
//   sparse:     Vec<u32>   — entity_id → индекс в dense.
//
//   Добавление: sparse[id] = dense.len(); dense.push(c).      O(1)
//   Удаление:  dense.swap_remove(i); sparse[that_id] = i.   O(1)
//   ⚠ `swap_remove` МЕНЯЕТ ПОРЯДОК. Если порядок важен — `remove(i)`
//   сдвигает всё, это O(n). Выбор между скоростью и порядком.
//
// ПОКОЛЕНИЯ (generational index). Когда сущность удаляется, её id
// освобождается. Если переиспользовать тот же номер, старые ссылки
// (например, «враг цели №7») станут невалидными, но молча укажут на
// новую сущность. Решение: id = (index, generation). Перед удалением
// generation++. Ссылка с неверным поколением отбрасывается.
// Это ровно то, что делает `Slab` в Vec и `SlotMap` в C++.
//
// СТРУКТУРНОЕ ИЗМЕНЕНИЕ ВО ВРЕМЯ ОБХОДА — главная боль ECS.
// Решения:
//   1. Command buffer: копишь изменения, применяешь после обхода.
//   2. Флаг `dirty` и обход с проверкой.
//   3. `swap_remove` в конце (порядок меняется).
// Мы сделаем (1) — это то, что делают почти все.

fn part_17_2() {
    harness::part("17.2", "ПРИМЕР: sparse set и обход");

    let mut world = MiniWorld::new();

    // Спавн: выдаём id = индекс слота
    let a = world.spawn(Pos { x: 0.0, y: 0.0, z: 0.0 });
    let b = world.spawn(Pos { x: 0.0, y: 5.0, z: 0.0 });
    let c = world.spawn(Pos { x: 0.0, y: -3.0, z: 0.0 });
    println!("  спавнены id = {a}, {b}, {c}");

    // Помечаем сущности
    world.insert_velocity(b, Velocity { x: 0.0, y: -9.8, z: 0.0 });
    world.insert_velocity(c, Velocity { x: 1.0, y: 0.0, z: 0.0 });
    println!("  у {} и {} есть гравитация", b, c);

    // СИСТЕМА ФИЗИКИ: обходит только те, у кого есть и Pos,
    // и velocity, и ПРИМЕНЯЕТ ГРАВИТАЦИЮ — один проход, ноль ветвлений
    // по типам.
    world.system_gravity(1.0);
    println!("  после гравитации (dt=1.0): b.y = {}, c.y = {}",
        world.pos(b).map(|t| t.y).unwrap_or(f32::NAN),
        world.pos(c).map(|t| t.y).unwrap_or(f32::NAN));
    assert!(world.pos(b).is_some_and(|t| t.y < 5.0), "гравитация применилась");

    // Удаление: O(1), порядок меняется
    world.despawn(a);
    println!("  удалён {a}, осталось {} живых", world.count());
    assert_eq!(world.count(), 2);

    // Удаление отсутствующего — не паника
    world.despawn(EntityId { index: 999, generation: 1 });
    println!("  despawn(несуществующий) — тихо, без паники");

    // --- Поколения ----------------------------------------------------------
    let id1 = world.spawn(Pos { x: 0.0, y: 0.0, z: 0.0 });
    let raw = world.spawn(Pos { x: 1.0, y: 1.0, z: 1.0 });
    println!("  id1 = {id1}, raw = {raw}");
    world.despawn(raw);
    let again = world.spawn(Pos { x: 2.0, y: 2.0, z: 2.0 });
    println!("  после удаления и повторного спавна: {again}");
    assert_ne!(again, raw, "поколение выросло — старый id больше невалиден");
    assert!(!world.is_alive(raw), "старый id помечен мёртвым");
    println!("    ⚠ без поколений снова был бы выдан тот же индекс, и старые ссылки ожили бы");

    // --- Структурное изменение во время обхода -------------------------------
    let mut w2 = MiniWorld::new();
    for i in 0..5u32 {
        let id = w2.spawn(Pos { x: 0.0, y: i as f32, z: 0.0 });
        w2.insert_velocity(id, Velocity { x: 0.0, y: -1.0, z: 0.0 });
    }
    // id = 0..4, скорость -1, dt = 1 → новая y = id - 1.
    // Порог kill_y = 0.0: упала только сущность с id = 0 (стала y = -1).
    let removed = w2.system_gravity_and_kill_below(1.0, 0.0);
    println!("  за один проход: гравитация + удаление {removed} шт. Осталось {}", w2.count());
    assert_eq!(w2.count(), 4);
    println!("    ⚠ Это возможно только потому, что удаление делается ПОСЛЕ обхода");
    println!("      (сначала собираем id в буфер, потом применяем).");
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Pos {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Velocity {
    x: f32,
    y: f32,
    z: f32,
}

/// SPARSE SET для одного типа компонента.
#[derive(Debug, Default)]
struct SparseSet<T> {
    /// Компоненты подряд — обход быстрый.
    dense: Vec<T>,
    /// entity index -> позиция в dense.
    sparse: Vec<usize>,
    /// Сущность на каждой позиции dense (нужно для despawn).
    owner: Vec<u32>,
    /// Удалённые слоты, которые можно переиспользовать.
    free: Vec<u32>,
}

const EMPTY: usize = usize::MAX;

impl<T: Copy> SparseSet<T> {
    fn new() -> Self {
        SparseSet { dense: Vec::new(), sparse: vec![EMPTY; 64], owner: Vec::new(), free: Vec::new() }
    }

    fn insert(&mut self, entity: u32, value: T) {
        let i = entity as usize;
        if i >= self.sparse.len() {
            self.sparse.resize(i + 1, EMPTY);
        }
        match self.sparse[i] {
            EMPTY => {
                let slot = self.free.pop().unwrap_or(self.dense.len() as u32);
                if slot as usize >= self.dense.len() {
                    self.dense.push(value);
                    self.owner.push(entity);
                } else {
                    self.dense[slot as usize] = value;
                    self.owner[slot as usize] = entity;
                }
                self.sparse[i] = slot as usize;
            }
            pos => {
                self.dense[pos] = value;
            }
        }
    }

    fn get(&self, entity: u32) -> Option<&T> {
        let i = entity as usize;
        let pos = *self.sparse.get(i)?;
        if pos == EMPTY { None } else { self.dense.get(pos) }
    }

    fn get_mut(&mut self, entity: u32) -> Option<&mut T> {
        let i = entity as usize;
        let pos = *self.sparse.get(i)?;
        if pos == EMPTY { None } else { self.dense.get_mut(pos) }
    }

    fn contains(&self, entity: u32) -> bool {
        self.get(entity).is_some()
    }

    /// O(1): последний элемент переезжает на место удаляемого.
    fn remove(&mut self, entity: u32) -> Option<T> {
        let i = entity as usize;
        let pos = *self.sparse.get(i)?;
        if pos == EMPTY {
            return None;
        }
        self.sparse[i] = EMPTY;
        let last = self.dense.len() - 1;
        if pos == last {
            let v = self.dense.pop()?;
            self.owner.pop();
            return Some(v);
        }
        // «Последний» переезжает в освободившееся место.
        self.dense[pos] = self.dense[last];
        self.owner[pos] = self.owner[last];
        self.sparse[self.owner[last] as usize] = pos;
        self.dense.pop();
        self.owner.pop();
        Some(self.dense[pos])
    }

    /// Обход в порядке dense: КЭШ-ДРУЖЕЛЮБНЫЙ.
    fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.owner.iter().zip(self.dense.iter()).map(|(o, v)| (*o, v))
    }

    fn len(&self) -> usize {
        self.dense.len()
    }
}

/// Мир с поколениями и command buffer'ом для структурных изменений.
struct MiniWorld {
    transforms: SparseSet<Pos>,
    velocities: SparseSet<Velocity>,
    next_index: u32,
    free_indices: Vec<u32>,
    generations: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EntityId {
    index: u32,
    generation: u32,
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}v{}", self.index, self.generation)
    }
}

impl MiniWorld {
    fn new() -> Self {
        MiniWorld {
            transforms: SparseSet::new(),
            velocities: SparseSet::new(),
            next_index: 0,
            free_indices: Vec::new(),
            generations: Vec::new(),
        }
    }

    fn spawn(&mut self, t: Pos) -> EntityId {
        let index = self.free_indices.pop().unwrap_or_else(|| {
            let i = self.next_index;
            self.next_index += 1;
            self.generations.push(1);
            i
        });
        self.transforms.insert(index, t);
        EntityId { index, generation: self.generations[index as usize] }
    }

    fn despawn(&mut self, id: EntityId) {
        if !self.is_alive(id) {
            return;
        }
        self.transforms.remove(id.index);
        self.velocities.remove(id.index);
        // ⚡ ПОКОЛЕНИЕ: старые ссылки с тем же index станут невалидными.
        self.generations[id.index as usize] += 1;
        self.free_indices.push(id.index);
    }

    fn is_alive(&self, id: EntityId) -> bool {
        (id.index as usize) < self.generations.len()
            && self.generations[id.index as usize] == id.generation
    }

    fn insert_velocity(&mut self, id: EntityId, v: Velocity) {
        self.velocities.insert(id.index, v);
    }

    fn pos(&self, id: EntityId) -> Option<Pos> {
        if self.is_alive(id) { self.transforms.get(id.index).copied() } else { None }
    }

    fn count(&self) -> usize {
        self.transforms.len()
    }

    /// СИСТЕМА ФИЗИКИ: один линейный проход, без проверок «есть ли компонент».
    fn system_gravity(&mut self, dt: f32) {
        // Собираем пары (индекс сущности, новый Pos) и применяем
        // после обхода: так структурное изменение не мешает обходу.
        let mut updates: Vec<(u32, Pos)> = Vec::with_capacity(self.transforms.len());
        for (idx, t) in self.transforms.iter() {
            if let Some(v) = self.velocities.get(idx) {
                updates.push((idx, Pos { x: t.x + v.x * dt, y: t.y + v.y * dt, z: t.z + v.z * dt }));
            }
        }
        for (idx, t) in updates {
            self.transforms.insert(idx, t);
        }
    }

    /// Тот же проход, но ещё и удаляет «упавшие». Именно поэтому
    /// сначала собираем, потом применяем.
    fn system_gravity_and_kill_below(&mut self, dt: f32, kill_y: f32) -> usize {
        let mut updates: Vec<(u32, Pos)> = Vec::new();
        let mut to_kill: Vec<u32> = Vec::new();
        for (idx, t) in self.transforms.iter() {
            if let Some(v) = self.velocities.get(idx) {
                let nt = Pos { x: t.x + v.x * dt, y: t.y + v.y * dt, z: t.z + v.z * dt };
                if nt.y < kill_y {
                    to_kill.push(idx);
                }
                updates.push((idx, nt));
            }
        }
        for (idx, t) in updates {
            self.transforms.insert(idx, t);
        }
        let n = to_kill.len();
        for idx in to_kill {
            self.transforms.remove(idx);
            self.velocities.remove(idx);
            self.generations[idx as usize] += 1;
            self.free_indices.push(idx);
        }
        n
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 17.3 — ТЕОРИЯ: AssetManager
// ─────────────────────────────────────────────────────────────────────────────
//
// ЗАДАЧА. Загрузить ассет по имени, но не каждый раз с диска.
// Отсюда три требования:
//
//   1. КЕШ: второй запрос того же имени не читает диск.
//   2. ДЕДУПЛИКАЦИЯ: два разных пути, ведущие к одному файлу
//      ("./assets/hero.mesh" и "assets/hero.mesh"), дают один AssetId.
//   3. ЛЕНИВОСТЬ: не грузить то, что не спросили.
//
// TYPE ERASURE. Ассеты разных типов лежат в одном кеше. Варианты:
//   * `HashMap<AssetId, Asset>` где `enum Asset { Mesh(..), Texture(..) }`
//     — типизировано, но новый тип ассета = правка enum (ломает match).
//   * `HashMap<AssetId, Box<dyn Any>>` + `downcast_ref::<T>()` —
//     расширяемо. ⚠ `Any` требует `'static`, поэтому ассет не может
//     содержать ссылки на внешнее (обычно это и нужно).
//   * `HashMap<AssetId, Arc<dyn Any>>` — плюс разделяемое владение.
//
// Мы используем (1) + downcast: это «пломбинг» (type-punning в Rust
// делается безопасно, в отличие от C++).
//
// HANDLE vs ID. ID — просто число, привязан к кешу. Handle — тоже
// число, но дополнительно содержит поколение кеша, чтобы «ручка из
// старого кеша» была отвергнута. Как в Sparse Set, только уровнем выше.

fn part_17_3() {
    harness::part("17.3", "ПРИМЕР: кеш, дедупликация, type erasure");

    let mut mgr = AssetManager::new();

    // Первая загрузка — читаем «с диска»
    let id1 = mgr.load("assets/hero.mesh", |path| {
        println!("    [диск] читаю {path}");
        Asset::Mesh(MeshData { vertices: 1024, name: "hero".into() })
    });
    // Вторая — из кеша
    let id2 = mgr.load("assets/hero.mesh", |_| {
        panic!("не должно читать диск");
    });
    assert_eq!(id1, id2);
    println!("  кеш сработал: id {id1:?} == {id2:?}, обращений к диску: {}", mgr.io_count());

    // Дедупликация путей
    let id3 = mgr.load("./assets/hero.mesh", |_| {
        println!("    [диск] читаю ./assets/hero.mesh — но это тот же файл!");
        Asset::Mesh(MeshData { vertices: 1024, name: "hero".into() })
    });
    println!("  дедупликация путей: \"./assets/hero.mesh\" → id {id3:?} (равен {id1:?}: {})", id1 == id3);
    assert_eq!(id1, id3, "нормализованный путь даёт тот же id");

    // Type erasure + downcast
    let any = mgr.get(id1).expect("есть");
    let mesh = any.downcast_ref::<Asset>().expect("это Asset");
    assert!(matches!(mesh, Asset::Mesh(_)));
    println!("  downcast_ref::<Asset>() — безопасно, в отличие от C++ type-punning");

    // Отсутствующий — None, не паника
    assert!(mgr.get(AssetId(999)).is_none());
    println!("  get(AssetId(999)) = None");

    // Статистика
    let s = mgr.stats();
    println!("  статистика: {} ассетов, {} попаданий, {} промахов", s.total, s.hits, s.misses);
    assert_eq!(s.total, 1);
    assert_eq!(s.hits, 2);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AssetId(u32);

#[derive(Debug, Clone, PartialEq)]
enum Asset {
    Mesh(MeshData),
    Texture(TextureData),
}

#[derive(Debug, Clone, PartialEq)]
struct MeshData {
    vertices: u32,
    name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct TextureData {
    w: u32,
    h: u32,
    format: u32,
}

#[derive(Debug, Default, Clone, Copy)]
struct CacheStats {
    total: usize,
    hits: u32,
    misses: u32,
}

struct AssetManager {
    /// Пломбинг: один кеш на все типы ассетов.
    assets: std::collections::HashMap<AssetId, std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    /// Нормализованный путь -> id. Это и есть дедупликация.
    by_path: std::collections::HashMap<String, AssetId>,
    next_id: u32,
    stats: CacheStats,
}

impl AssetManager {
    fn new() -> Self {
        AssetManager {
            assets: std::collections::HashMap::new(),
            by_path: std::collections::HashMap::new(),
            next_id: 0,
            stats: CacheStats::default(),
        }
    }

    /// Классический «get or insert»: без `clone` пути, без двойного поиска.
    fn load<T, F>(&mut self, path: &str, loader: F) -> AssetId
    where
        T: std::any::Any + Send + Sync,
        F: FnOnce(&str) -> T,
    {
        let key = normalize_path(path);
        if let Some(&id) = self.by_path.get(&key) {
            self.stats.hits += 1;
            return id;
        }
        self.stats.misses += 1;
        let id = AssetId(self.next_id);
        self.next_id += 1;
        self.assets.insert(id, std::sync::Arc::new(loader(path)));
        self.by_path.insert(key, id);
        self.stats.total += 1;
        id
    }

    fn get(&self, id: AssetId) -> Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        self.assets.get(&id)
    }

    fn io_count(&self) -> u32 {
        self.stats.misses
    }

    fn stats(&self) -> CacheStats {
        self.stats
    }
}

/// Нормализация пути: убираем `./`, двойные слеши, приводим слеши.
/// В реальном проекте — `path_clean` crate, но 10 строк своего кода
/// показывают, что «нормализация» это просто строковая операция.
fn normalize_path(p: &str) -> String {
    let unified = p.replace('\\', "/");
    let mut out = String::with_capacity(unified.len());
    let mut prev_slash = false;
    for ch in unified.chars() {
        if ch == '/' {
            if prev_slash {
                continue;
            }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        if ch == '.' && !out.is_empty() {
            // ведущий "./" отбрасываем целиком
            if out.is_empty() || out == "./" {
                continue;
            }
        }
        out.push(ch);
    }
    let out = out.trim_start_matches("./").to_string();
    if out.is_empty() { "/".to_string() } else { out }
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 17.4 — ЗАДАНИЕ 17.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Формат сцены, который переживёт изменение структуры. Требования
/// сформулированы как «что будет, если»:
///
///   * добавить поле в `Entity` → старые файлы должны открываться;
//   * старый код должен открывать новые файлы → нужен `version`
///     и поведение «неизвестное поле пропустить»;
///   * файл обрезан посередине → `Err`, а не паника.
///
/// ```ignore
/// fn write_scene(s: &Scene) -> Result<Vec<u8>, EncodeError>;
/// fn read_scene(b: &[u8]) -> Result<Scene, DecodeError>;
/// fn upgrade(scene: Scene, from: u16) -> Scene;   // миграция версий
/// ```
///
/// Требования:
///   * Заголовок: `"NOVE"` (4) + version (u16) + flags (u16) +
///     имя (u32 len + bytes) + count (u32).
///   * `DecodeError` — enum, НЕ Option: «не наш файл», «обрезан»,
///     «версия новее, чем умеем», «мусор в имени».
///   * `upgrade` поднимает версию 1 → 2 (например, добавляет
///     новое поле с дефолтом). Проверь round-trip v1 → upgrade → v2.
///   * `read_scene` на мусоре → `Err`, не паника. На обрезанном → `Err`.
///   * Посчитай `alloc::measure` для записи: сколько аллокаций?
///     Подсказка: используй `Vec::with_capacity` и `extend_from_slice`.
fn task_17_1() {
    use curriculum::alloc;

    #[derive(Debug, PartialEq)]
    enum DecodeError {
        NotOurFile,
        Truncated,
        TooNew { found: u16, supported: u16 },
        BadUtf8,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct EntityV2 {
        id: u32,
        x: f32,
        y: f32,
        z: f32,
        /// Новое поле, которого не было в v1.
        layer: u8,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct SceneV2 {
        name: String,
        entities: Vec<EntityV2>,
    }

    fn write_scene(s: &SceneV2) -> Result<Vec<u8>, EncodeError> {
        not_yet!("заголовок + сущности; подсчитай размер заранее и with_capacity");
    }

    fn read_scene(b: &[u8]) -> Result<SceneV2, DecodeError> {
        not_yet!("проверяй магию, версию, длины; никаких unwrap");
    }

    /// Миграция: v1 (без layer) -> v2 (layer = 0).
    fn upgrade_from_v1(name: String, ids: &[u32]) -> SceneV2 {
        not_yet!("layer = 0 по умолчанию");
    }

    // Собираем файл v1 «руками», чтобы проверить миграцию.
    let mut v1: Vec<u8> = Vec::new();
    v1.extend_from_slice(b"NOVE");
    v1.extend_from_slice(&1u16.to_le_bytes());
    v1.extend_from_slice(&0u16.to_le_bytes());
    v1.extend_from_slice(&3u32.to_le_bytes());
    v1.extend_from_slice(b"lvl1");
    v1.extend_from_slice(&2u32.to_le_bytes());
    for id in [10u32, 20] {
        v1.extend_from_slice(&id.to_le_bytes());
        v1.extend_from_slice(&1.5f32.to_le_bytes());
        v1.extend_from_slice(&2.5f32.to_le_bytes());
        v1.extend_from_slice(&3.5f32.to_le_bytes());
    }

    // ⚠ read_scene должен отвергнуть v1 (слишком старая) ИЛИ принять
    //   и вызвать upgrade. Выбери политику и напиши почему.
    match read_scene(&v1) {
        Ok(s) => {
            assert_eq!(s.entities.len(), 2);
            assert!(s.entities.iter().all(|e| e.layer == 0), "миграция проставила дефолт");
        }
        Err(DecodeError::TooNew { .. }) => panic!("неверная классификация ошибки"),
        Err(e) => {
            // Допустимо: отвергли как «старую версию». Тогда проверяем
            // ручную миграцию.
            println!("  read_scene отверг v1: {e:?} — проверяем ручную миграцию");
            let upgraded = upgrade_from_v1("lvl1".into(), &[10, 20]);
            assert_eq!(upgraded.entities.len(), 2);
        }
    }

    // Битые файлы
    assert!(matches!(read_scene(&[0, 0, 0, 0, 1, 0]), Err(DecodeError::NotOurFile)));
    assert!(matches!(read_scene(&[]), Err(DecodeError::Truncated)));
    let mut v2future = v1.clone();
    v2future[4..6].copy_from_slice(&99u16.to_le_bytes());
    assert!(matches!(read_scene(&v2future), Err(DecodeError::TooNew { found: 99, .. })));

    // Round-trip v2
    let scene = SceneV2 {
        name: "lvl1".into(),
        entities: vec![
            EntityV2 { id: 10, x: 1.0, y: 2.0, z: 3.0, layer: 0 },
            EntityV2 { id: 20, x: 4.0, y: 5.0, z: 6.0, layer: 1 },
        ],
    };
    let bytes = write_scene(&scene).expect("запись");
    let back = read_scene(&bytes).expect("чтение");
    assert_eq!(back, scene, "round-trip");

    let (_, allocs, _) = alloc::measure(|| { std::hint::black_box(write_scene(&scene).expect("запись")); });
    println!("  запись сцены из {} сущностей: {allocs} аллокаций", scene.entities.len());
}

/// ЧАСТЬ 17.5 — ЗАДАНИЕ 17.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Мини-ECS с настоящими требованиями. Три системы, один обход.
///
/// ```ignore
/// struct World { /* SparseSet<Pos>, SparseSet<Velocity>, ... */ }
///
/// impl World {
///     fn spawn(&mut self, t: Pos) -> EntityId;
///     fn despawn(&mut self, id: EntityId);
///     fn is_alive(&self, id: EntityId) -> bool;
///     fn get<T: ComponentId>(&self, id: EntityId) -> Option<&T>;
///     fn set<T: ComponentId>(&mut self, id: EntityId, v: T);
///     fn system_gravity(&mut self, dt: f32);
///     fn system_clamp_y(&mut self, min: f32, max: f32);
///     fn system_ai(&mut self);                 // использует Tag
///     fn count(&self) -> usize;
///     fn component_count<T: ComponentId>(&self) -> usize;
/// }
/// ```
///
/// Требования:
///   * `ComponentId` — маркерный трейт с `const ID: u16` и
///     `const NAME: &'static str`. `SparseSet<T>` становится
///     `SparseSetStore` через enum-диспетчер (объясни, почему
///     `Vec<Box<dyn Any>>` здесь хуже).
///   * `system_clamp_y` идёт ВТОРЫМ шагом и использует результат
///     гравитации: это демонстрация зависимости систем.
///   * `system_ai` ходит по всем с Pos, у которых есть
///     `Tag("enemy")`, и двигает их к «игроку». Значит нужны
///     `SparseSet<Tag>` и два обхода.
///   * Поколения обязательны: `despawn` + `spawn` должен выдать
///     НОВЫЙ id с новым поколением.
///   * Проверь, что `component_count::<Pos>() == count()`.
fn task_17_2() {
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Pos {
        x: f32,
        y: f32,
        z: f32,
    }
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Velocity {
        x: f32,
        y: f32,
        z: f32,
    }
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Tag(&'static str);

    // ⚠ Обобщённый доступ к компонентам через ТРЕЙТ с ассоциированной
    //   константой. Хранилище — enum, а не Vec<Box<dyn Any>>:
    //   enum даёт статическую диспетчеризацию (вызов инлайнится) и
    //   не требует Any'а; Box<dyn Any> дал бы vtable на каждый компонент
    //   и запретил бы non-'static данные.
    trait ComponentId: Copy {
        const ID: u16;
        const NAME: &'static str;
    }
    impl ComponentId for Pos {
        const ID: u16 = 1;
        const NAME: &'static str = "Pos";
    }
    impl ComponentId for Velocity {
        const ID: u16 = 2;
        const NAME: &'static str = "Velocity";
    }
    impl ComponentId for Tag {
        const ID: u16 = 3;
        const NAME: &'static str = "Tag";
    }

    const EMPTY: usize = usize::MAX;

    #[derive(Debug, Default)]
    struct SparseSet<T> {
        dense: Vec<T>,
        sparse: Vec<usize>,
        owner: Vec<u32>,
    }

    impl<T: Copy> SparseSet<T> {
        fn insert(&mut self, e: u32, v: T) {
            let i = e as usize;
            if i >= self.sparse.len() {
                self.sparse.resize(i + 1, EMPTY);
            }
            if self.sparse[i] == EMPTY {
                self.sparse[i] = self.dense.len();
                self.dense.push(v);
                self.owner.push(e);
            } else {
                self.dense[self.sparse[i]] = v;
            }
        }
        fn get(&self, e: u32) -> Option<&T> {
            let p = *self.sparse.get(e as usize)?;
            if p == EMPTY { None } else { self.dense.get(p) }
        }
        fn get_mut(&mut self, e: u32) -> Option<&mut T> {
            let p = *self.sparse.get(e as usize)?;
            if p == EMPTY { None } else { self.dense.get_mut(p) }
        }
        fn remove(&mut self, e: u32) {
            let i = e as usize;
            let Some(p) = self.sparse.get(i).copied() else { return };
            if p == EMPTY {
                return;
            }
            self.sparse[i] = EMPTY;
            let last = self.dense.len() - 1;
            if p != last {
                self.dense.swap(p, last);
                self.owner.swap(p, last);
                self.sparse[self.owner[p] as usize] = p;
            }
            self.dense.pop();
            self.owner.pop();
        }
        fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
            self.owner.iter().zip(self.dense.iter()).map(|(o, v)| (*o, v))
        }
        fn len(&self) -> usize {
            self.dense.len()
        }
    }

    /// Хранилище всех типов компонентов: enum, а не полиморфизм.
    ///
    /// ⚠ У SparseSet здесь НЕТ `#[derive(Default)]`: derive требует
    ///   `Default` у T, а компонентам (Pos, Velocity) он не нужен —
    ///   им нечего «по умолчанию». Конструктор пишется руками.
    ///   Это частая ловушка derive: он навешивает требования на все
    ///   параметры типа, даже не нужные.
    struct Components {
        transform: SparseSet<Pos>,
        velocity: SparseSet<Velocity>,
        tag: SparseSet<Tag>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct EntityId {
        index: u32,
        generation: u32,
    }

    struct World {
        components: Components,
        generations: Vec<u32>,
        free: Vec<u32>,
        next: u32,
    }

    impl World {
        fn new() -> Self {
            not_yet!("пустые компоненты, generations пуст");
        }
        fn spawn(&mut self, t: Pos) -> EntityId {
            not_yet!("возьми free или новый index, вставь Pos, верни id");
        }
        fn despawn(&mut self, id: EntityId) {
            not_yet!("удали из ВСЕХ наборов, generation++, free.push");
        }
        fn is_alive(&self, id: EntityId) -> bool {
            not_yet!("generations[index] == id.generation");
        }
        fn get<T: ComponentId>(&self, id: EntityId) -> Option<&T> {
            not_yet!("match T::ID и достай из нужного набора");
        }
        fn set<T: ComponentId>(&mut self, id: EntityId, v: T) {
            not_yet!("то же на запись");
        }
        fn system_gravity(&mut self, dt: f32) {
            not_yet!("обход + буфер обновлений");
        }
        fn system_clamp_y(&mut self, min: f32, max: f32) {
            not_yet!("ВТОРОЙ шаг поверх первого");
        }
        fn system_ai(&mut self) {
            not_yet!("найди Tag(\"enemy\") и Tag(\"player\"), двигай enemy к player");
        }
        fn count(&self) -> usize {
            not_yet!("Pos.len()");
        }
        fn component_count<T: ComponentId>(&self) -> usize {
            not_yet!("match T::ID");
        }
    }

    let mut w = World::new();
    let hero = w.spawn(Pos { x: 0.0, y: 100.0, z: 0.0 });
    let e1 = w.spawn(Pos { x: 50.0, y: 100.0, z: 0.0 });
    let coin = w.spawn(Pos { x: 5.0, y: 100.0, z: 0.0 });
    w.set(e1, Velocity { x: 0.0, y: -9.8, z: 0.0 });
    w.set(e1, Tag("enemy"));
    w.set(hero, Tag("player"));

    w.system_gravity(1.0);
    w.system_clamp_y(0.0, 150.0);
    w.system_ai();

    assert_eq!(w.component_count::<Pos>(), 3);
    assert!(w.is_alive(hero));
    assert!(w.get::<Tag>(e1) == Some(&Tag("enemy")));
    assert!(w.get::<Tag>(coin).is_none());

    let raw = w.spawn(Pos { x: 0.0, y: 0.0, z: 0.0 });
    w.despawn(raw);
    let reused = w.spawn(Pos { x: 0.0, y: 0.0, z: 0.0 });
    assert_ne!(raw, reused, "поколение изменилось");
    assert!(!w.is_alive(raw));
    assert!(w.get::<Pos>(raw).is_none(), "старый id не достаёт компонент");
}

/// ЧАСТЬ 17.6 — ЗАДАНИЕ 17.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// `AssetManager` с ленивой загрузкой и дефолтами. Отличие от примера
/// в части 17.3 — Handle с поколением кеша: при перезагрузке всех
/// ассетов старые handle'ы должны стать невалидными, а не указывать
/// на новые данные.
///
/// ```ignore
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// struct AssetId(u32);
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// struct Handle { id: AssetId, generation: u32 }
///
/// struct AssetManager { /* ... */ }
///
/// impl AssetManager {
///     fn new() -> Self;
///     fn get_or_load<T: Any + Send + Sync>(&mut self, path: &str, f: impl FnOnce() -> T) -> Handle;
///     fn resolve<T: Any + Send + Sync>(&self, h: Handle) -> Option<&T>;
///     fn with_default<T: Default + Any + Send + Sync>(&mut self, path: &str) -> Handle;
///     fn reload_all(&mut self);            // generation++, кеш пуст
///     fn is_valid(&self, h: Handle) -> bool;
///     fn stats(&self) -> CacheStats;
/// }
/// ```
///
/// Требования:
///   * Нормализация путей (как в примере), дедупликация.
///   * `get_or_load` НЕ грузит, если есть в кеше; `with_default`
///     кладёт `T::default()` без обращения к «диску».
///   * `reload_all` инвалидирует все handle'ы: `resolve` на старый
///     handle → `None`, а не новые данные.
///   * `resolve` делает `downcast_ref` и возвращает `None`, если тип
///     не тот. ⚠ Объясни, почему это безопаснее, чем `transmute` в C++.
///   * Посчитай аллокации: `HashMap::with_capacity` + `entry` должен
///     дать ровно одну аллокацию на вставку.
fn task_17_3() {
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct AssetId(u32);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Handle {
        id: AssetId,
        generation: u32,
    }

    #[derive(Debug, Default, Clone, Copy)]
    struct CacheStats {
        pub total: usize,
        pub hits: u32,
        pub misses: u32,
    }

    struct AssetManager {
        data: HashMap<AssetId, Arc<dyn Any + Send + Sync>>,
        by_path: HashMap<String, AssetId>,
        next_id: u32,
        generation: u32,
        stats: CacheStats,
    }

    impl AssetManager {
        fn new() -> Self {
            not_yet!("пустые карты, generation = 1");
        }

        fn get_or_load<T: Any + Send + Sync>(
            &mut self,
            path: &str,
            f: impl FnOnce() -> T,
        ) -> Handle {
            not_yet!("проверь by_path (hit), иначе загрузи и вставь (miss)");
        }

        fn with_default<T: Default + Any + Send + Sync>(&mut self, path: &str) -> Handle {
            not_yet!("то же, но f = T::default(); используй entry-идиому");
        }

        fn resolve<T: Any + Send + Sync>(&self, h: Handle) -> Option<&T> {
            not_yet!("generation != h.generation -> None, потом downcast_ref");
        }

        fn reload_all(&mut self) {
            not_yet!("generation += 1; data.clear(); stats.total = 0");
        }

        fn is_valid(&self, h: Handle) -> bool {
            not_yet!("generation == h.generation");
        }

        fn stats(&self) -> CacheStats {
            self.stats
        }
    }

    fn normalize(path: &str) -> String {
        not_yet!("./ и двойные слеши в одну, слеши одинаковые");
    }

    let mut m = AssetManager::new();
    let h1 = m.get_or_load("assets/tex.png", || 42u32);
    let h2 = m.get_or_load("./assets/tex.png", || panic!("не грузим повторно"));
    assert_eq!(h1, h2, "дедупликация");
    assert_eq!(m.resolve::<u32>(h1), Some(&42));
    assert_eq!(m.resolve::<String>(h1), None, "не тот тип — None, не паника");
    assert!(m.is_valid(h1));

    let d = m.with_default::<String>("assets/empty.txt");
    assert_eq!(m.resolve::<String>(d), Some(&String::new()));

    assert!(m.resolve::<u32>(Handle { id: AssetId(999), generation: 1 }).is_none());

    // Перезагрузка инвалидирует handle'ы
    m.reload_all();
    assert!(!m.is_valid(h1), "старый handle невалиден после reload");
    assert_eq!(m.resolve::<u32>(h1), None);
    assert_eq!(m.resolve::<String>(d), None);

    // Ссылка на тот же путь после reload получает НОВЫЙ handle
    let h3 = m.get_or_load("assets/tex.png", || 7u32);
    assert_ne!(h1, h3);
    assert_eq!(m.resolve::<u32>(h3), Some(&7));

    let s = m.stats();
    println!("  статистика: {} ассетов, {} хитов, {} промахов", s.total, s.hits, s.misses);
    assert!(s.misses >= 1);
}
