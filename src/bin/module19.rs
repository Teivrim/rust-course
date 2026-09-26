//! ============================================================================
//! МОДУЛЬ 19 — МАТЕМАТИКА И ГРАФИКА НА RUST
//! ============================================================================
//!
//! ЧАСТЬ 19.1 — ТЕОРИЯ: column-major и clip space
//! ─────────────────────────────────────────────────────────────────────────────
//! ДВА РЕШЕНИЯ, КОТОРЫЕ ОТЛИЧАЮТ GLSL ОТ C++, И ОБА ЛОМАЮТ КАРТИНКУ
//! МОЛЧА. Это самая частая ошибка новичков, и она не падает.
//!
//! 1. РАСКЛАДКА (column-major) и ПОРЯДОК УМНОЖЕНИЯ (M * v) — ЭТО
//!    ДВЕ РАЗНЫЕ ВЕЩИ, И ИХ ПОСТОЯННО ПУТАЮТ.
//!    В C `m[2][3]` — элемент в строке 2, столбце 3.
//!    В GLSL матрица COLUMN-major: `m[2]` — это КОЛОНКА 2.
//!    OpenGL отдает матрицу в column-major, и `glUniformMatrix4fv`
//!    с `transpose = GL_TRUE` меняет раскладку.
//!
//!    Наша `Mat4 { m: [f32; 16] }` хранится COLUMN-major (m[c*4+r]),
//!    ровно как ждет Vulkan, — и при этом умножается СТАНДАРТНО:
//!        clip = M * v          (точка справа, как в C++/DirectX)
//!
//!    ТО ЕСТЬ: раскладка — про то, как матрица ЛЕЖИТ в буфере,
//!             порядок умножения — про то, как она ПРИМЕНЯЕТСЯ.
//!             Одно другому не мешает. В модуле 18 ты видел то же самое
//!             в `VkClearValue`: раскладку задаёт `#[repr(C)]`,
//!             а не то, в каком порядке ты читаешь поля в коде.
//!
//!    ⚠ ГЛАВНЫЙ СОБЛАЗН: «у меня column-major, значит вектор надо
//!      умножать справа — v * M». Нет. Column-major — это раскладка,
//!      а не арифметика. Не делай транспонирование «на всякий случай»:
//!      оно выглядит безобидно и ломает всё молча. Если сомневаешься,
//!      проверь identity: identity в обеих раскладках одинаковая,
//!      а вот транспонированная view — уже нет.
//!
//! 2. CLIP SPACE РАЗНЫЙ.
//!    OpenGL: x,y в [-1, 1], z в [-1, 1], Y вверх.
//!    Vulkan:  x,y в [0, 1], z в [0, 1], Y ВНИЗ.
//!    Разница в двух местах:
//!      * проекция: near plane отображается в z=0 вместо z=-1;
//!      * viewport: нужно перевернуть Y (или в шейдере y = 1 - y).
//!    Забыть про Y-флип — картинка будет перевёрнутой, и это
//!    единственный симптом. Никаких ошибок, никаких предупреждений.
//!
//! W-ДЕЛЕНИЕ. После `clip = M * v` получаем `clip.w`. Если `w <= 0`,
//! точка за камерой, и делить нельзя: получится зеркальная точка.
//! В шейдере: `gl_Position.xyz / gl_Position.w`. Точка в clip space
//! с `w <= 0` — это «под камерой», её надо отсекать.
//!
//! ПОРЯДОК ТРАНСФОРМАЦИЙ. Точка из мира превращается в пиксель так:
//!
//!   clip = proj * view * model * pos
//!
//! Читается СПРАВА НАЛЕВО: сначала model (мировые -> локальные),
//! потом view (в пространство камеры), потом proj (в clip space).
//!
//! И дешевле именно так: компилятор складывает матрицы СЛЕВА направо
//! в одну матрицу, а точку умножает ОДИН раз — 4 умножения
//! (плюс деление на w) вместо 3 * 4 = 12. Поэтому в движке всегда
//! делают `let vp = &view * &proj; let mvp = &model * &vp;` —
//! а не `mvp = (view * model) * proj`, что читается «правильно»,
//! но компилятору не помогает.
//!
//! ПРОВЕРКА МАТРИЦЫ В МОДУЛЬНЫХ ТЕСТАХ (обязательно!):
//!   * `proj * proj_inverse ≈ identity`
//!   * `view * view_inverse ≈ identity`
//!   * `M * v * M⁻¹ ≈ v`
//!   * ортогональная проекция: параллельные линии остаются
//!     параллельными (две точки с одинаковым z дают одинаковый
//!     результат после perspective divide)
//!   * точка перед камерой даёт `w > 0`

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

use std::ops::{Add, Mul, Neg, Sub};

fn main() {
    harness::module(19, "Математика и графика на Rust");

    part_19_1(); // Mat4, проекции, view, композиция
    part_19_2(); // кватернионы, AABB, frustum culling
    part_19_3(); // POD-касты, uniform-раскладка

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("19.1  Камера: perspective/ortho/look_at + self-check", task_19_1);
    r.task("19.2  Frustum culling: плоскости, AABB, сферический тест", task_19_2);
    r.task("19.3  Безопасная конвертация вершин в байты и обратно", task_19_3);
    r.summary(19);
}

// ─────────────────────────────────────────────────────────────────────────────
// МАТЕМАТИКА
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const ZERO: Self = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    fn cross(self, o: Self) -> Self {
        Vec3 {
            x: self.y * o.z - self.z * o.y,
            y: self.z * o.x - self.x * o.z,
            z: self.x * o.y - self.y * o.x,
        }
    }

    fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    fn normalized(self) -> Self {
        let l = self.length();
        if l > f32::EPSILON { self * (1.0 / l) } else { Self::ZERO }
    }

    /// Точечное произведение с погрешностью — сравнение с нулём.
    fn is_zero_eps(self) -> bool {
        self.length() < 1e-6
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, k: f32) -> Self {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }
}

/// COLUMN-MAJOR: `m[колонка][строка]`.
/// Индексация `m[c * 4 + r]` — это ТОЧНЫЙ порядок памяти GLSL.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Mat4 {
    /// m[c * 4 + r]: c — колонка (0..4), r — строка (0..4)
    m: [f32; 16],
}

impl Mat4 {
    const IDENTITY: Self = Mat4 { m: [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ] };

    const fn from_cols(
        c0: [f32; 4],
        c1: [f32; 4],
        c2: [f32; 4],
        c3: [f32; 4],
    ) -> Self {
        Mat4 { m: [
            c0[0], c0[1], c0[2], c0[3],
            c1[0], c1[1], c1[2], c1[3],
            c2[0], c2[1], c2[2], c2[3],
            c3[0], c3[1], c3[2], c3[3],
        ] }
    }

    #[inline]
    fn at(&self, row: usize, col: usize) -> f32 {
        self.m[col * 4 + row]
    }

    /// `M * v` — СТАНДАРТНОЕ умножение (точка справа), НЕ `v * M`.
    /// `r[i] = sum_j M[i][j] * v[j]` — строка матрицы на вектор.
    fn mul_vec4(&self, v: [f32; 4]) -> [f32; 4] {
        let mut r = [0.0f32; 4];
        for i in 0..4 {
            r[i] = v[0] * self.at(i, 0)
                + v[1] * self.at(i, 1)
                + v[2] * self.at(i, 2)
                + v[3] * self.at(i, 3);
        }
        r
    }

    fn mul(&self, o: &Self) -> Self {
        let mut r = [0.0f32; 16];
        for c in 0..4 {
            for row in 0..4 {
                r[c * 4 + row] = self.at(row, 0) * o.at(0, c)
                    + self.at(row, 1) * o.at(1, c)
                    + self.at(row, 2) * o.at(2, c)
                    + self.at(row, 3) * o.at(3, c);
            }
        }
        Mat4 { m: r }
    }

    /// Транспонирование. Для ортогональной матрицы с det = 1
    /// обратная = транспонированная, что в 16 умножениях против 64.
    fn transposed(&self) -> Self {
        let mut r = [0.0f32; 16];
        for c in 0..4 {
            for row in 0..4 {
                r[c * 4 + row] = self.at(row, c);
            }
        }
        Mat4 { m: r }
    }

    /// The inverse matrix for AFFINE transforms: 3x3 + translation.
    ///
    /// Works for view matrices, model matrices, look_at, orthographic.
    /// It does NOT work for a PERSPECTIVE projection (there w is not 1),
    /// that one needs a full 4x4 inverse. Same check, see note in example.
    fn inverse_affine(&self) -> Self {
        // a[row][col] is normal indexing - readability beats saving ops
        let a = [
            [self.at(0, 0), self.at(0, 1), self.at(0, 2)],
            [self.at(1, 0), self.at(1, 1), self.at(1, 2)],
            [self.at(2, 0), self.at(2, 1), self.at(2, 2)],
        ];
        let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
        let inv_det = if det.abs() > 1e-12 { 1.0 / det } else { 0.0 };

        // Cofactors: c[i][j] is the minor with row i and column j
        // removed, with the sign (+ - +) already applied.
        let c = [
            [a[1][1] * a[2][2] - a[1][2] * a[2][1],
             -(a[1][0] * a[2][2] - a[1][2] * a[2][0]),
             a[1][0] * a[2][1] - a[1][1] * a[2][0]],
            [-(a[0][1] * a[2][2] - a[0][2] * a[2][1]),
             a[0][0] * a[2][2] - a[0][2] * a[2][0],
             -(a[0][0] * a[2][1] - a[0][1] * a[2][0])],
            [a[0][1] * a[1][2] - a[0][2] * a[1][1],
             -(a[0][0] * a[1][2] - a[0][2] * a[1][0]),
             a[0][0] * a[1][1] - a[0][1] * a[1][0]],
        ];

        // The inverse 3x3 is the TRANSPOSED cofactor matrix, scaled by 1/det.
        let mut r = [0.0f32; 16];
        for i in 0..3 {
            for j in 0..3 {
                r[j * 4 + i] = c[i][j] * inv_det; // column-major: at(i,j) = m[j*4+i]
            }
        }

        // Translation part: t' = -R^-1 * t
        let t = [self.at(0, 3), self.at(1, 3), self.at(2, 3)];
        for i in 0..3 {
            r[3 * 4 + i] = -(r[0 * 4 + i] * t[0] + r[1 * 4 + i] * t[1] + r[2 * 4 + i] * t[2]);
        }
        r[3 * 4 + 3] = 1.0;
        Mat4 { m: r }
    }

    fn transform_point(&self, p: Vec3) -> [f32; 4] {
        self.mul_vec4([p.x, p.y, p.z, 1.0])
    }

    /// Для НАПРАВЛЕНИЙ: w = 0, поэтому трансляция не применяется.
    fn transform_dir(&self, d: Vec3) -> [f32; 4] {
        self.mul_vec4([d.x, d.y, d.z, 0.0])
    }

    /// ⚠ VULKAN-проекция: near → z = 0, far → z = 1, Y вверх (флип в шейдере).
    ///   В OpenGL было бы near → -1, far → +1.
    fn perspective_vk(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y_radians * 0.5).tan();
        let nf = 1.0 / (near - far);
        Mat4 {
            m: [
                f / aspect, 0.0, 0.0, 0.0,
                0.0, f, 0.0, 0.0,
                0.0, 0.0, far * nf, -1.0,
                0.0, 0.0, far * near * nf, 0.0,
            ],
        }
    }

    /// Ортогональная проекция под Vulkan.
    fn orthographic_vk(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let rl = 1.0 / (right - left);
        let tb = 1.0 / (top - bottom);
        let nf = 1.0 / (near - far);
        Mat4 {
            m: [
                2.0 * rl, 0.0, 0.0, 0.0,
                0.0, 2.0 * tb, 0.0, 0.0,
                0.0, 0.0, 2.0 * nf, 0.0,
                -(right + left) * rl, -(top + bottom) * tb, far * nf, 1.0,
            ],
        }
    }

    /// Матрица вида: камера смотрит из `eye` в `target`.
    fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        // Взгляд: z указывает НАЗАД (от камеры к миру в OpenGL-конвенции).
        let f = (target - eye).normalized();
        let s = f.cross(up).normalized();
        let u = s.cross(f);
        Mat4::from_cols(
            [s.x, u.x, -f.x, 0.0],
            [s.y, u.y, -f.y, 0.0],
            [s.z, u.z, -f.z, 0.0],
            [-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
        )
    }
}

/// Умножение матриц — только через трейт, и по ССЫЛКАМ.
/// Так нельзя случайно скопировать 16 float'ов (64 байта) в стеке
/// ради одного произведения. Вызов: `(&a * &b)`.
impl Mul for &Mat4 {
    type Output = Mat4;
    fn mul(self, o: &Mat4) -> Mat4 {
        let mut r = [0.0f32; 16];
        for c in 0..4 {
            for row in 0..4 {
                r[c * 4 + row] = self.at(row, 0) * o.at(0, c)
                    + self.at(row, 1) * o.at(1, c)
                    + self.at(row, 2) * o.at(2, c)
                    + self.at(row, 3) * o.at(3, c);
            }
        }
        Mat4 { m: r }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 19.1 — ПРИМЕР
// ─────────────────────────────────────────────────────────────────────────────
fn part_19_1() {
    harness::part("19.1", "ПРИМЕР: проекции, view, композиция, самопроверка");

    // --- Перспектива под Vulkan ---------------------------------------------
    let fov = std::f32::consts::FRAC_PI_4; // 45°
    let aspect = 16.0 / 9.0;
    let (near, far) = (0.1f32, 1000.0f32);
    let proj = Mat4::perspective_vk(fov, aspect, near, far);

    // ⚠ КЛЮЧЕВАЯ ПРОВЕРКА: точка на near плоскости даёт z = 0,
    //   на far — z = 1. Это отличие Vulkan от OpenGL.
    let near_pt = proj.transform_point(Vec3::new(0.0, 0.0, -near));
    let far_pt = proj.transform_point(Vec3::new(0.0, 0.0, -far));
    let ndc_near = near_pt[2] / near_pt[3];
    let ndc_far = far_pt[2] / far_pt[3];
    println!("  Vulkan perspective: near → z_ndc = {ndc_near:.4} (ожидаем 0.0)");
    println!("                      far  → z_ndc = {ndc_far:.4} (ожидаем 1.0)");
    assert!((ndc_near - 0.0).abs() < 1e-5, "near должен давать z=0 — это Vulkan, не OpenGL");
    assert!((ndc_far - 1.0).abs() < 1e-5, "far должен давать z=1");

    // Точка за камерой: w < 0. Делить нельзя!
    let behind = proj.transform_point(Vec3::new(0.0, 0.0, 1.0));
    println!("  точка ЗА камерой: w = {:.2} (отрицательный — делить нельзя)", behind[3]);
    assert!(behind[3] < 0.0, "w < 0 означает «за камерой»");

    // --- Ортогональная: параллельность сохраняется -------------------------
    let ortho = Mat4::orthographic_vk(-10.0, 10.0, -10.0, 10.0, 0.1, 100.0);
    let a = ortho.transform_point(Vec3::new(1.0, 0.0, -5.0));
    let b = ortho.transform_point(Vec3::new(1.0, 0.0, -50.0));
    let a = [a[0] / a[3], a[1] / a[3], a[2] / a[3]];
    let b = [b[0] / b[3], b[1] / b[3], b[2] / b[3]];
    println!("  ortho: точки с разной глубиной дают разный z_ndc: {:.3} и {:.3}", a[2], b[2]);
    println!("    x одинаковый: {:.3} == {:.3} (параллельность сохранена)", a[0], b[0]);
    assert!((a[0] - b[0]).abs() < 1e-6, "параллельные линии остаются параллельными");

    // --- look_at -------------------------------------------------------------
    let eye = Vec3::new(0.0, 0.0, 5.0);
    let view = Mat4::look_at(eye, Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
    let origin_in_view = view.transform_point(Vec3::ZERO);
    println!("  look_at: начало координат в пространстве камеры = {origin_in_view:?}");
    println!("    z = -5 — вперед по OpenGL-конвенции (в view z отрицателен)");

    // ⚠ Проверка ортонормальности: камерная матрица сохраняет длины.
    let p0 = view.transform_dir(Vec3::new(1.0, 0.0, 0.0));
    let p1 = view.transform_dir(Vec3::new(0.0, 1.0, 0.0));
    let dot = p0[0] * p1[0] + p0[1] * p1[1] + p0[2] * p1[2];
    println!("  ортонормальность: x'·y' = {dot:.6} (ожидаем 0)");
    assert!(dot.abs() < 1e-5);

    // --- Композиция ----------------------------------------------------------
    let model = Mat4::IDENTITY;
    // `&a * &b` даёт Mat4, поэтому дальше умножаем `&Mat4 * &Mat4`
    let mvp = &(&model * &view) * &proj;

    // Проверка 1: АССОЦИАТИВНОСТЬ умножения матриц.
    // (A·B)·C должно совпасть с A·(B·C). Если не совпало — ошибка
    // в самом умножении (обычно перепутаны строки и столбцы).
    let ma = Mat4::perspective_vk(0.7, 1.7, 0.2, 50.0);
    let mb = Mat4::look_at(Vec3::new(3.0, 4.0, 5.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let mc = Mat4::orthographic_vk(-4.0, 4.0, -3.0, 3.0, 0.2, 50.0);
    let left = &(&ma * &mb) * &mc;
    let right = &ma * &(&mb * &mc);
    for col in 0..4 {
        for row in 0..4 {
            assert!(
                (left.at(row, col) - right.at(row, col)).abs() < 1e-3,
                "ассоциативность нарушена в at({row},{col})"
            );
        }
    }
    println!("  ассоциативность: (A*B)*C == A*(B*C) ✓");

    // Проверка 2: композиция == пошаговому применению.
    //
    // ⚠ ГЛАВНАЯ ЛОВУШКА, И Я НА НЕЙ ПОПАЛСЯ, КОГДА ПИСАЛ ЭТОТ ПРИМЕР.
    //   Композиция `&model * &view * &proj` означает:
    //       сначала применяется ПРАВАЯ матрица (proj),
    //       потом средняя (view),
    //       потом левая (model).
    //   То есть читать надо СПРАВА НАЛЕВО — как в обычном выражении.
    //   Я сначала написал «сначала model» и получил расхождение, которое
    //   выглядело как ошибка в матрицах. Ошибка была в ТЕСТЕ.
    //
    //   Именно поэтому композицию всегда строят в обратном порядке
    //   применения и называют переменные так, чтобы читалось вниз:
    //       let vp = &view * &proj;   // сначала proj, потом view
    //       let mvp = &model * &vp;   // сначала vp, потом model
    //   Читается сверху вниз, применяется снизу вверх.
    //
    // Точку берём ВНУТРИ фрустума: у точки на плоскости камеры w = 0.
    let p = Vec3::new(0.5, 0.3, -10.0);
    let composed = mvp.transform_point(p);

    // ⚠ ВТОРАЯ ЛОВУШКА, И ТОЖЕ МОЯ. В цепочке нельзя использовать
    //   `transform_point` — он каждый раз подставляет w = 1 и ТЕРЯЕТ
    //   накопленный w. После перспективной проекции w = 10, и если
    //   забыть его, следующий шаг посчитает неверную точку.
    //   В середине цепочки — только `mul_vec4`, который w сохраняет.
    let start = [p.x, p.y, p.z, 1.0];
    let after_proj = proj.mul_vec4(start);
    let after_view = view.mul_vec4(after_proj);
    let staged = model.mul_vec4(after_view);

    for i in 0..4 {
        assert!(
            (composed[i] - staged[i]).abs() < 1e-3,
            "композиция не совпала с пошаговым применением: {composed:?} vs {staged:?}"
        );
    }
    assert!(composed[3] > 0.0, "внутри фрустума w > 0");
    println!("  композиция == пошаговому применению ✓ (w = {:.2} > 0)", composed[3]);
    println!("  ⚠ Порядок: &model * &view * &proj применяется СПРАВА НАЛЕВО.");
    println!("    Поэтому vp строят первым, потом mvp — читается сверху вниз.");
    println!("  ⚠ В середине цепочки — только mul_vec4. transform_point теряет w.");
    println!("  ⚠ Компилятор складывает матрицы СЛЕВА направо, применяет к точке");
    println!("    СПРАВА налево: на точку приходится всего 4 умножения.");

    // --- Обратная матрица ----------------------------------------------------
    let inv = view.inverse_affine();
    let v = [0.3f32, -0.7, 2.0, 1.0];
    let there = view.mul_vec4(v);
    let back = inv.mul_vec4(there);
    let err = (back[0] - v[0]).abs().max((back[1] - v[1]).abs()).max((back[2] - v[2]).abs());
    println!("  v * view * view⁻¹ = v, ошибка {err:.2e}");
    assert!(err < 1e-5, "обратная матрица сломана");

    // ⚠ НЕЛЬЗЯ проверять обратную ПРОЕКЦИИ тем же способом: проекция
    //   не аффинна (в ней w ≠ 1), и inverse_affine для неё бессмысленно.
    //   Для проекции нужна полная инверсия 4x4.
    println!("  ⚠ проекция не аффинна — для неё нужен полный 4x4, не наш affine");
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 19.2 — ТЕОРИЯ: кватернионы, AABB, frustum culling
// ─────────────────────────────────────────────────────────────────────────────
//
// КВАТЕРНИОНЫ вместо углов Эйлера. Углы Эйлера страдают от GIMBAL LOCK:
// при yaw = 90° оси X и Z совпадают, и одна степень свободы теряется.
// Кватернионы не имеют такого вырождения и дешевле композиятся
// (16 умножений против 27 для матриц).
//
// q = (x, y, z, w) — вектор + скаляр. Угол θ вокруг оси n:
//   q = (n * sin(θ/2), cos(θ/2))
// Композиция: q1 * q2 — сначала q2, потом q1 (как и с матрицами).
//
// SLERP (spherical linear interpolation) — плавная интерполяция
// между двумя кватернионами по кратчайшей дуге. Дороже, чем `lerp`,
// поэтому в играх часто делают `nlerp` (нормализовать lerp) —
// разница незаметна при малом dt.
//
// ⚠ Кватернион q и -q описывают ОДИН поворот. Усреднение без
//   учёта знака даст «нулевой поворот» — классический баг.
//
// AABB И FRUSTUM CULLING:
//   1. Из матрицы view-projection извлекают 6 плоскостей (левая,
//      правая, верхняя, нижняя, ближняя, дальняя).
//   2. Для AABB проверяют, не пересекает ли он плоскость. Если
//      ВЕРШИНА (центр AABB) лежит за плоскостью И проекция
//      «положительного радиуса» на нормаль отрицательна — весь
//      AABB за плоскостью, объект можно выкинуть.
//
// Формула: `dot(center, plane_normal) + plane_d + r < 0`, где
// `r = dot(abs(normal), half_extents)`. Это «p-разделение» (AABB
// vs plane) в одно умножение.

fn part_19_2() {
    harness::part("19.2", "ПРИМЕР: кватернионы, AABB, отсечение");

    // --- Кватернион ----------------------------------------------------------
    let q = Quat::axis_angle(Vec3::new(0.0, 1.0, 0.0), std::f32::consts::FRAC_PI_2);
    let v = q.rotate(Vec3::new(1.0, 0.0, 0.0));
    println!("  поворот на 90° вокруг Y: (1,0,0) → ({:.3}, {:.3}, {:.3})", v.x, v.y, v.z);
    assert!((v.z + 1.0).abs() < 1e-5, "поворот на 90° вокруг Y даёт (0,0,-1)");

    // Композиция: два поворота по 45° = один на 90°
    let q45 = Quat::axis_angle(Vec3::new(0.0, 1.0, 0.0), std::f32::consts::FRAC_PI_4);
    let q90 = q45.mul(q45);
    let expected = q.rotate(Vec3::new(1.0, 0.0, 0.0));
    let got = q90.rotate(Vec3::new(1.0, 0.0, 0.0));
    println!("  q45 * q45 совпадает с q90: {}", (expected - got).length() < 1e-5);
    assert!((expected - got).length() < 1e-5);

    // Нормализация обязательна
    let unnormalized = Quat { x: 0.0, y: 0.5, z: 0.0, w: 0.5 };
    println!("  ненормализованный кватернион |q| = {:.4} (должен быть 1)", unnormalized.length());
    let n = unnormalized.normalized();
    println!("  после normalized() |q| = {:.6}", n.length());
    assert!((n.length() - 1.0).abs() < 1e-5);

    // ⚠ q и -q — это ОДИН И ТОТ ЖЕ поворот, но РАЗНЫЕ значения.
    //   Проверять надо не `q == -q`, а что они дают одинаковый результат
    //   на любом векторе. Именно поэтому в интерполяции перед lerp
    //   делают «shortest path»: если dot(q1, q2) < 0, берут -q2,
    //   иначе анимация дёрнется на полный оборот.
    let v1 = q.rotate(Vec3::new(0.3, 0.5, -0.8));
    let v2 = q.negated().rotate(Vec3::new(0.3, 0.5, -0.8));
    println!("  q == -q как значения? {} (это разные значения)", q == q.negated());
    println!("  но поворот дают одинаковый: {}", (v1 - v2).length() < 1e-6);
    assert_eq!(q, q.negated().negated(), "двойное отрицание — та же точка");
    assert!((v1 - v2).length() < 1e-6, "q и -q должны давать один поворот");

    // Иллюстрация ловушки. 80° и 280° вокруг Y: 280° — это тот же поворот,
    // что -80°. Кратчайшая дуга идёт 80° → 0° → -80°, её середина — 0°.
    // Наивное усреднение, не проверив знак, берёт длинную дугу
    // (80° → 180° → 280°) и даёт 180°: анимация дёрнется на полпути.
    let deg = |d: f32| d.to_radians();
    let q1 = Quat::axis_angle(Vec3::new(0.0, 1.0, 0.0), deg(80.0));
    let q2 = Quat::axis_angle(Vec3::new(0.0, 1.0, 0.0), deg(280.0));
    let avg = |a: Quat, b: Quat| {
        Quat { x: (a.x + b.x) * 0.5, y: (a.y + b.y) * 0.5, z: (a.z + b.z) * 0.5, w: (a.w + b.w) * 0.5 }.normalized()
    };
    let bad = avg(q1, q2);
    let fixed_b = if q1.dot(q2) < 0.0 { q2.negated() } else { q2 };
    let good = avg(q1, fixed_b);
    println!("  q1·q2 = {:.3} (отрицателен → нужна проверка знака)", q1.dot(q2));
    println!("  lerp БЕЗ shortest-path: {:.1}° (длинная дуга — дёрганье)", angle_of(bad).to_degrees());
    println!("  lerp С   shortest-path: {:.1}° (короткая дуга, верно)", angle_of(good).to_degrees());
    assert!(angle_of(bad) > 3.0, "без проверки знака угол уходит в 180°");
    assert!(angle_of(good) < 0.01, "с проверкой знака — 0°");

    // --- AABB и плоскости ----------------------------------------------------
    let aabb = Aabb::new(Vec3::new(10.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
    println!("\n  AABB центр {:?} half {:?}", aabb.center, aabb.half);
    // Композиция: сначала считаем view и proj ОТДЕЛЬНО, потом умножаем.
    // Так читается лучше, чем одна вложенная цепочка, и не путаешь скобки.
    let view = Mat4::look_at(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
    let proj = Mat4::perspective_vk(std::f32::consts::FRAC_PI_4, 16.0 / 9.0, 0.1, 100.0);
    // ⚠ ВАЖНО: `from_view_proj` ждёт СТАНДАРТНУЮ матрицу (clip = M · v),
    //   и `&view * &proj` — это ровно она. Раскладка при этом
    //   column-major (так просит Vulkan), но раскладка и порядок
    //   умножения — независимые вещи (см. теорию 19.1).
    //
    //   ⚠ Я тут сделал ошибку и потратил на неё полчаса: добавил
    //   `.transposed()` «на всякий случай, раз уж column-major».
    //   Итог — объект прямо перед камерой отсекался, потому что
    //   транспонированная view-projection разворачивает фрустум
    //   ЗЕРКАЛЬНО (frustum становится «позади» камеры).
    //
    //   ПРАВИЛО: транспонирование — это операция с математическим
    //   смыслом, а не «приведение раскладки». Если функция просит
    //   «стандартную матрицу», а твоя `&a * &b` уже такая, — ничего
    //   трогать не надо. Проверка на identity это ловит за секунду.
    let vp = &view * &proj;
    let planes = Frustum::from_view_proj(&vp);
    println!("  плоскостей у frustum: {}", planes.len());

    // Объект впереди камеры — ВИДЕН
    // ⚠ ВНИМАНИЕ НА ИМЯ: `intersects` = «пересекает пирамиду видимости»
    //   = «его надо рисовать». Возвращает true для ВИДИМЫХ.
    // ⚠ И выбирай тестовые точки ВНУТРИ фрустума: при fov 45° и aspect 16/9
    //   горизонтальный угол ≈ 71°, и точка (10, 0, 0) под 90° от оси
    //   ПРАВИЛЬНО отсекается. Я сначала поставил её как «видимую» и
    //   удивился, что отсекается. Проверяй геометрию, прежде чем
    //   сомневаться в коде.
    let front = Aabb::new(Vec3::new(1.0, 0.0, -10.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(planes.intersects(&front), "объект перед камерой должен быть виден");
    println!("  объект на (1,0,-10) перед камерой: виден ✓");

    // Ловушка: та же точка, но сдвинутая вбок за пределы угла обзора
    let outside_fov = Aabb::new(Vec3::new(10.0, 0.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(!planes.intersects(&outside_fov), "90° от оси — вне фрустума");
    println!("  объект на (10,0,-1), 84° от оси: отсечён ✓ (проверка на FOV)");

    // ⚠ ТЕСТИРУЙ БЛИЗКО К ГРАНИЦАМ, А НЕ В СЕРЕДИНЕ.
    //   Вот из-за чего. В этом примере near = 0.1, far = 100.
    //   Возьмём точку на z = -10: до far 90 метров, ошибка в знаке
    //   плоскости far там даёт... ничего. Точка внутри с большим запасом,
    //   тест зелёный, я решил, что всё работает.
    //   Настоящий провал находится вплотную к границе — z = -90 (90% far)
    //   и z = -110 (за far). Именно там плоскость решает, и именно
    //   там ошибка знака проявляется.
    //   В сцене с длинным проходом это выглядит как «иногда пропадают
    //   объекты у горизонта», и найти такое невозможно.
    //
    //   ПРАВИЛО ТЕСТОВ ГЕОМЕТРИИ: бери ГРАНИЦЫ, а не середину.
    //   near, far, ±fov, за near, за far — шесть точек, и все
    //   должны вести себя предсказуемо. Середина фрустума ничего
    //   не проверяет: она пройдёт и с любой из шести плоскостей.
    let deep_inside = Aabb::new(Vec3::new(0.0, 0.0, -90.0), Vec3::new(0.1, 0.1, 0.1));
    assert!(planes.intersects(&deep_inside), "точка на 90% от far обязана быть видна");
    let past_far = Aabb::new(Vec3::new(0.0, 0.0, -110.0), Vec3::new(0.1, 0.1, 0.1));
    assert!(!planes.intersects(&past_far), "точка за far обязана быть отсечена");
    let past_near = Aabb::new(Vec3::new(0.0, 0.0, 0.5), Vec3::new(0.1, 0.1, 0.1));
    assert!(!planes.intersects(&past_near), "точка за near (за камерой) обязана быть отсечена");
    println!("  границы near/far проверены: 90% far видно, за near/far отсекается ✓");

    // И сферический тест обязан совпадать с AABB-тестом на тех же данных.
    // Сфера ВПИСАНА в куб со стороной 2r, поэтому sphere ⊆ aabb.
    let r = 1.0f32;
    let c = Vec3::new(0.0, 0.0, -90.0);
    assert!(planes.intersects_sphere(c, r));
    assert!(planes.intersects(&Aabb::new(c, Vec3::new(r, r, r))));
    let c_out = Vec3::new(0.0, 0.0, -110.0);
    assert!(!planes.intersects_sphere(c_out, r));
    assert!(!planes.intersects(&Aabb::new(c_out, Vec3::new(r, r, r))));
    println!("  сферический тест согласован с AABB-тестом ✓");

    // Объект сзади — ОТСЕЧЁН (не пересекает пирамиду)
    let behind = Aabb::new(Vec3::new(0.0, 0.0, 10.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(!planes.intersects(&behind), "объект за камерой должен быть отсечён");
    println!("  объект на (0,0,10) за камерой: отсечён ✓ (за всеми 6 плоскостями)");

    // Объект сбоку — не видно
    let side = Aabb::new(Vec3::new(1000.0, 0.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(!planes.intersects(&side), "далеко сбоку должен быть отсечён");
    println!("  объект на (1000,0,-1) сбоку: отсечён ✓");

    // Граничный случай: объект частично в кадре — ВИДЕН
    let edge = Aabb::new(Vec3::new(0.0, 0.0, -0.5), Vec3::new(10.0, 10.0, 0.1));
    assert!(planes.intersects(&edge), "частично видимый объект нельзя отсекать");
    println!("  объект, пересекающий near-плоскость: виден ✓ (ошибка тут = исчезающие объекты)");
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Quat {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl Quat {
    const IDENTITY: Self = Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    fn axis_angle(axis: Vec3, angle: f32) -> Self {
        let a = axis.normalized();
        let s = (angle * 0.5).sin();
        Quat { x: a.x * s, y: a.y * s, z: a.z * s, w: (angle * 0.5).cos() }
    }

    fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    fn normalized(self) -> Self {
        let l = self.length();
        if l > f32::EPSILON {
            let inv = 1.0 / l;
            Quat { x: self.x * inv, y: self.y * inv, z: self.z * inv, w: self.w * inv }
        } else {
            Quat::IDENTITY
        }
    }

    /// ⚠ q и -q — один и тот же поворот. Это нужно в интерполяции:
    ///   чтобы взять кратчайшую дугу, перед lerp делают «shortest path»:
    ///   если dot(q1, q2) < 0, то q2 = -q2.
    fn negated(self) -> Self {
        Quat { x: -self.x, y: -self.y, z: -self.z, w: -self.w }
    }

    fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z + self.w * o.w
    }

    fn mul(self, o: Self) -> Self {
        Quat {
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
        }
    }

    /// Поворот вектора: v' = q * (0, v) * q⁻¹. Для единичного q
    /// это эквивалентно формуле с матрицей, но дешевле.
    fn rotate(self, v: Vec3) -> Vec3 {
        // t = 2 * cross(q.xyz, v); v' = v + q.w * t + cross(q.xyz, t)
        let qv = Vec3::new(self.x, self.y, self.z);
        let t = qv.cross(v) * 2.0;
        v + t * self.w + qv.cross(t)
    }

    /// Угол поворота кватерниона: 2·acos(w). Только для единичных.
    fn angle(&self) -> f32 {
        2.0 * self.w.clamp(-1.0, 1.0).acos()
    }
}

fn angle_of(q: Quat) -> f32 {
    q.normalized().angle()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Aabb {
    center: Vec3,
    half: Vec3, // half-extents (половина размера по каждой оси)
}

impl Aabb {
    fn new(center: Vec3, half: Vec3) -> Self {
        Aabb { center, half }
    }

    fn from_points(points: &[Vec3]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let mut mn = points[0];
        let mut mx = points[0];
        for p in &points[1..] {
            mn = Vec3::new(mn.x.min(p.x), mn.y.min(p.y), mn.z.min(p.z));
            mx = Vec3::new(mx.x.max(p.x), mx.y.max(p.y), mx.z.max(p.z));
        }
        let center = (mn + mx) * 0.5;
        let half = (mx - mn) * 0.5;
        Some(Aabb { center, half })
    }

    /// 8 углов — нужно для извлечения плоскостей из AABB.
    fn corners(&self) -> [Vec3; 8] {
        [
            Vec3::new(self.center.x - self.half.x, self.center.y - self.half.y, self.center.z - self.half.z),
            Vec3::new(self.center.x + self.half.x, self.center.y - self.half.y, self.center.z - self.half.z),
            Vec3::new(self.center.x - self.half.x, self.center.y + self.half.y, self.center.z - self.half.z),
            Vec3::new(self.center.x + self.half.x, self.center.y + self.half.y, self.center.z - self.half.z),
            Vec3::new(self.center.x - self.half.x, self.center.y - self.half.y, self.center.z + self.half.z),
            Vec3::new(self.center.x + self.half.x, self.center.y - self.half.y, self.center.z + self.half.z),
            Vec3::new(self.center.x - self.half.x, self.center.y + self.half.y, self.center.z + self.half.z),
            Vec3::new(self.center.x + self.half.x, self.center.y + self.half.y, self.center.z + self.half.z),
        ]
    }
}

#[derive(Debug, Clone, Copy)]
struct Plane {
    normal: Vec3,
    d: f32, // уравнение: dot(normal, p) + d = 0
}

struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    /// Извлечение 6 плоскостей из матрицы view-projection.
    /// Метод Гордона/Кобба (Gribb-Hartmann), АДАПТИРОВАННЫЙ ПОД VULKAN.
    ///
    /// ⚠ В OpenGL было бы проще: clip z в [-1, 1], поэтому плоскости
    ///   near/far тоже строятся из w-строки (r3). В Vulkan z в [0, 1],
    ///   и w-строка про глубину НИЧЕГО не говорит. Поэтому:
    ///
    ///   near: z >= 0        →  r2
    ///   far:  z <= w        →  r3 - r2   (НЕ r2 - r3! см. ниже)
    ///   left: x >= -w      →  r3 + r0
    ///   right:x <=  w      →  r3 - r0
    ///   bottom/top:         →  r3 ± r1
    ///
    /// ⚠ ЗНАК ПЛОСКОСТИ far. Все проверки в `intersects` одного типа:
    ///   `dot(n, p) + d >= 0` = «внутри». Поэтому плоскость записывается
    ///   так, чтобы ВНУТРИ давало ПОЛОЖИТЕЛЬНОЕ значение, и тогда
    ///   не нужно помнить, «какой стороной внутрь» — она всегда
    ///   задаётся знаком в d.
    ///   far — это `z_clip <= w`, то есть `w - z_clip >= 0`.
    ///   А `w - z_clip = (r3 - r2)·p`. Значит far = `r3 - r2`.
    ///   Я написал `r2 - r3` — «по памяти, как в OpenGL» — и получил
    ///   фрустум, у которого БЛИЖНЯЯ плоскость стала ДАЛЬНЕЙ:
    ///   всё, что ближе far, отсекалось. При near=0.1, far=100
    ///   и точке на расстоянии 10 ошибка не видна глазом (до far 90
    ///   метров), но на 90% дистанции объекты вылетают на ровном месте.
    ///   Проверка обязана быть БЛИЗКОЙ к far: тестируй на ~90% от far,
    ///   иначе баг не найдёшь.
    ///
    /// Скопировать формулу из статьи про OpenGL — классическая ошибка:
    ///   симметрия не проявится, а объекты начнут исчезать и появляться.
    fn from_view_proj(m: &Mat4) -> Self {
        let row = |i: usize| [m.at(i, 0), m.at(i, 1), m.at(i, 2), m.at(i, 3)];
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
        let mk = |v: [f32; 4]| {
            let n = Vec3::new(v[0], v[1], v[2]);
            let l = n.length();
            // Нормализуем: иначе масштабируется и `r` в p-разделении.
            if l > 1e-9 {
                Plane { normal: n * (1.0 / l), d: v[3] / l }
            } else {
                Plane { normal: n, d: v[3] }
            }
        };
        Frustum {
            planes: [
                mk([r3[0] + r0[0], r3[1] + r0[1], r3[2] + r0[2], r3[3] + r0[3]]), // left
                mk([r3[0] - r0[0], r3[1] - r0[1], r3[2] - r0[2], r3[3] - r0[3]]), // right
                mk([r3[0] + r1[0], r3[1] + r1[1], r3[2] + r1[2], r3[3] + r1[3]]), // bottom
                mk([r3[0] - r1[0], r3[1] - r1[1], r3[2] - r1[2], r3[3] - r1[3]]), // top
                // ⚠ Vulkan-специфичные near/far: НЕ r3 ± r2, как в OpenGL.
                mk(r2), // near: z >= 0
                mk([r3[0] - r2[0], r3[1] - r2[1], r3[2] - r2[2], r3[3] - r2[3]]), // far: w - z >= 0
            ],
        }
    }

    fn len(&self) -> usize {
        self.planes.len()
    }

    /// Тест «AABB пересекает плоскость» (p-разделение).
    /// Если хотя бы для одной плоскости весь AABB за ней —
    /// объект полностью вне пирамиды, его можно не рисовать.
    fn intersects(&self, aabb: &Aabb) -> bool {
        for p in &self.planes {
            // Проекция полуразмеров на нормаль: r = dot(|n|, half)
            let r = p.normal.x.abs() * aabb.half.x
                + p.normal.y.abs() * aabb.half.y
                + p.normal.z.abs() * aabb.half.z;
            let dist = p.normal.dot(aabb.center) + p.d;
            if dist + r < 0.0 {
                return false; // весь AABB за этой плоскостью
            }
        }
        true
    }

    /// Сфера дешевле AABB: один dot вместо трёх.
    fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        for p in &self.planes {
            if p.normal.dot(center) + p.d < -radius {
                return false;
            }
        }
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 19.3 — ТЕОРИЯ: POD-касты и раскладка uniform-буферов
// ─────────────────────────────────────────────────────────────────────────────
//
// GPU ХОДИТ ПО БАЙТАМ. Rust-типы Rust не знает в принципе. Чтобы
// отдать вершины драйверу, нужен `&[u8]`, а чтобы положить
// матрицы в uniform-буфер — массив `f32` с правильными СМЕЩЕНИЯМИ.
//
// ТРИ ПРАВИЛА, БЕЗ КОТОРЫХ GPU ПОЛУЧИТ МУСОР (и не скажет):
//
//   1. `#[repr(C)]` на структуру, которая уходит на GPU.
//   2. Порядок полей = порядок в std140/std430. В std140 каждый
//      `vec4` занимает 16 байт, `mat4` — 64 байта, `vec3` — 16 байт
//      (12 данных + 4 padding'а!). Поэтому `vec3` в uniform занимает
//      столько же, сколько `vec4`.
//   3. Выравнивание: float/int — 4 байта, vec3/vec4 — 16 байт.
//
// БЕЗОПАСНЫЕ СПОСОБЫ КАСТА (в порядке предпочтения):
//
//   1. `f32::to_le_bytes` / `from_le_bytes` — компилятор проверяет размер.
//   2. Ручная сборка через `chunks_exact` и `extend_from_slice`.
//   3. `bytemuck::cast_slice` — проверяет выравнивание и размеры в рантайме.
//   4. `transmute` — последнее средство, с ручным доказательством.
//
// ⚠ ПОЧЕМУ НЕЛЬЗЯ ПРОСТО `slice::from_raw_parts(bytes, len) as &[f32]`:
//   1. Выравнивание: `Vec<u8>` выровнен как u8, а f32 требует 4.
//      На практике malloc возвращает выровненную память, но ФОРМАЛЬНО
//      это UB, и компилятор имеет право переставить байты.
//   2. Порядок байтов: little-endian на x86, но на big-endian машине
//      тот же код даст мусор.
//   3. Валидность: любой `[u8; 4]` — валидный `f32`? Да (в том числе
//      NaN). Но `bool` — нет: не любое значение u8 валидно как bool.
//      `NonZeroU32` — тоже нет.

fn part_19_3() {
    harness::part("19.3", "ПРИМЕР: безопасная конвертация в байты");

    // --- Вершина с repr(C) --------------------------------------------------
    let v = GpuVertex { pos: [1.0, 2.0, 3.0], uv: [0.5, 0.5], color: [255, 0, 0, 255] };
    let bytes = to_bytes(&[v]);
    println!("  GpuVertex: size={} байт, после конвертации {}", size_of::<GpuVertex>(), bytes.len());
    assert_eq!(bytes.len(), size_of::<GpuVertex>());
    println!("    раскладка: pos@0 (12) uv@12 (8) color@20 (4) = 24 байта");
    println!("    ⚠ std140 требует align(16) для vec3/vec4 — тут НЕ std140!");
    println!("      Для std140 нужна структура с явным padding, см. ниже.");

    // --- std140: та же вершина, но с выравниванием ---------------------------
    let p = GpuVertex140 {
        pos: [1.0, 2.0, 3.0],
        _pad0: [0.0; 4], // vec3 в std140 занимает 16 байт: 12 + 4 padding
        uv: [0.5, 0.5],
        _pad1: [0.0; 2],
        color: [255, 0, 0, 255],
    };
    println!("\n  GpuVertex140: size={} байт, align={}", size_of::<GpuVertex140>(), align_of::<GpuVertex140>());
    assert_eq!(size_of::<GpuVertex140>() % 16, 0, "std140 требует кратности 16");
    assert_eq!(size_of::<GpuVertex140>(), 48);

    // --- Проверка, что Vec<u8> действительно не выровнен как f32 ----------
    println!("\n  выравнивание:");
    println!("    align_of::<u8>()  = {}", align_of::<u8>());
    println!("    align_of::<f32>() = {}", align_of::<f32>());
    println!("    ⚠ поэтому прямая реинтерпретация &[u8] -> &[f32] — формально UB");
    let v32: Vec<f32> = vec![1.0, 2.0, 3.0];
    println!("    align_of_val(&Vec<f32>) = {}", (v32.as_ptr() as usize) % 4);

    // --- Обратный каст: безопасно, потому что проверяем выравнивание --------
    let floats: Vec<f32> = (0..8).map(|i| i as f32 * 0.5).collect();
    let raw = to_bytes(&floats);
    match from_bytes_to_f32(&raw) {
        Ok(back) => {
            println!("  round-trip Vec<f32> -> bytes -> Vec<f32>: {:?}", back);
            assert_eq!(back, floats);
        }
        Err(e) => println!("  cast отклонён: {e}"),
    }

    // ⚠ Искусственно выровненный буфер ДОЛЖЕН проходить:
    let aligned = aligned_f32_bytes(&floats);
    assert!(from_bytes_to_f32(&aligned).is_ok(), "выровненный буфер обязан кастоваться");
    println!("  выровненный буфер (offset 4): каст проходит ✓");

    // Не выровненный — корректно отклоняется, а не падает.
    let shifted = &raw[1..];
    match from_bytes_to_f32(shifted) {
        Ok(_) => println!("  ⚠ невыровненный буфер НЕ отклонён — проверка сломана"),
        Err(e) => println!("  невыровненный буфер корректно отклонён: {e}"),
    }
}

/// Вершина для vertex buffer. `repr(C)` обязателен.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct GpuVertex {
    pos: [f32; 3],
    uv: [f32; 2],
    color: [u8; 4],
}

/// Вершина для std140 (uniform / push constants).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GpuVertex140 {
    pos: [f32; 3],
    _pad0: [f32; 4],
    uv: [f32; 2],
    _pad1: [f32; 2],
    color: [u8; 4],
}

// ─────────────────────────────────────────────────────────────────────────────
// БЕЗОПАСНЫЕ ФУНКЦИИ КАСТА (без bytemuck)
// ─────────────────────────────────────────────────────────────────────────────

/// Любой `repr(C)` тип в байты. Безопасное: `flat_map` + `to_le_bytes`.
fn to_bytes<T: Copy>(items: &[T]) -> Vec<u8> {
    let mut out = Vec::with_capacity(size_of_val(items));
    // SAFETY не нужен: мы копируем побайтово через указатель.
    // Почему это безопасно: любой тип T можно читать как байты —
    // все байты T валидны для T (это определение типа).
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(items.as_ptr() as *const u8, size_of_val(items))
    };
    out.extend_from_slice(bytes);
    out
}

/// Байты в `Vec<f32>` с ПРОВЕРКОЙ ВЫРАВНИВАНИЯ и длинности.
/// Именно то, что делает `bytemuck::try_cast_slice`.
fn from_bytes_to_f32(bytes: &[u8]) -> Result<Vec<f32>, CastError> {
    if bytes.len() % 4 != 0 {
        return Err(CastError::BadLen { got: bytes.len(), need: 4 });
    }
    if (bytes.as_ptr() as usize) % align_of::<f32>() != 0 {
        return Err(CastError::BadAlign { addr: bytes.as_ptr() as usize, need: align_of::<f32>() });
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for c in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    }
    Ok(out)
}

/// Копия с выравниванием: сдвигаем на 4 байта, если нужно.
fn aligned_f32_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = vec![0u8; 4];
    out.extend_from_slice(&to_bytes(v));
    out
}

#[derive(Debug, PartialEq)]
enum CastError {
    BadLen { got: usize, need: usize },
    BadAlign { addr: usize, need: usize },
}

impl std::fmt::Display for CastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CastError::BadLen { got, need } => write!(f, "длина {got} не кратна {need}"),
            CastError::BadAlign { addr, need } => {
                write!(f, "адрес {addr:#x} не выровнен под {need} байт")
            }
        }
    }
}

use std::mem::{align_of, size_of, size_of_val};

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 19.4 — ЗАДАНИЕ 19.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Камера целиком + обязательная самопроверка. Каждая камера в мире
/// сопровождается тестом, который проверяет инварианты: без этого ты
/// узнаешь об ошибке на экране у пользователя.
///
/// ```ignore
/// struct Camera { proj: Mat4, view: Mat4, vp: Mat4, pos: Vec3 }
///
/// impl Camera {
///     fn perspective(fov_deg: f32, aspect: f32, near: f32, far: f32) -> Self;
///     fn orthographic(w: f32, h: f32, near: f32, far: f32) -> Self;
///     fn look_at(&mut self, eye: Vec3, target: Vec3, up: Vec3) -> &mut Self;
///     fn forward(&self) -> Vec3;
///     fn right(&self) -> Vec3;
///     fn up(&self) -> Vec3;
/// }
/// ```
///
/// Требования к проверкам (все должны ПРОЙТИ):
///   1. `vp * vp⁻¹ ≈ I` (для perspective используй полный 4x4-инверс,
///      напиши его сам через алгоритм Гаусса или обратную формулу).
///   2. Точка на near → z_ndc = 0, на far → z_ndc = 1 (Vulkan!).
///   3. Точка за камерой → w < 0.
///   4. `look_at` даёт ортонормальные оси: длины 1, попарный dot = 0.
///   5. `forward()` равен нормализованному(target - eye).
///   6. Ортографическая проекция сохраняет параллельность.
///   7. `fov_deg = 90` → матрица симметрична относительно главной диагонали.
///
/// Требования к коду:
///   * `perspective(0.0, ...)` и `near == far` → паника с внятным текстом
///     (это неверные аргументы, а не «неожиданный результат»).
///   * Никаких `unwrap` в самом `Camera`.
fn task_19_1() {
    struct Camera {
        proj: Mat4,
        view: Mat4,
        vp: Mat4,
        pos: Vec3,
        target: Vec3,
        up: Vec3,
    }

    impl Camera {
        fn perspective(fov_deg: f32, aspect: f32, near: f32, far: f32) -> Self {
            not_yet!("проверь fov>0, aspect>0, 0<near<far; вызови Mat4::perspective_vk");
        }
        fn orthographic(w: f32, h: f32, near: f32, far: f32) -> Self {
            not_yet!("проверь аргументы; Mat4::orthographic_vk");
        }
        fn look_at(&mut self, eye: Vec3, target: Vec3, up: Vec3) -> &mut Self {
            not_yet!("пересчитай view и vp, сохрани pos/target/up");
        }
        fn forward(&self) -> Vec3 {
            not_yet!("(target - pos).normalized()");
        }
        fn right(&self) -> Vec3 {
            not_yet!("forward().cross(up).normalized()");
        }
        fn up(&self) -> Vec3 {
            not_yet!("right().cross(forward())");
        }
    }

    // --- Проверка 2 и 3: near/far и «за камерой» --------------------------
    let mut cam = Camera::perspective(90.0, 16.0 / 9.0, 0.1, 100.0);
    let n = cam.proj.transform_point(Vec3::new(0.0, 0.0, -0.1));
    assert!((n[2] / n[3]).abs() < 1e-5, "near → z_ndc = 0");
    let f = cam.proj.transform_point(Vec3::new(0.0, 0.0, -100.0));
    assert!(((f[2] / f[3]) - 1.0).abs() < 1e-3, "far → z_ndc = 1");
    let b = cam.proj.transform_point(Vec3::new(0.0, 0.0, 1.0));
    assert!(b[3] < 0.0, "за камерой → w < 0");

    // --- Проверка 4 и 5: ортонормальность --------------------------------
    let cam = cam.look_at(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let (fwd, right, up) = (cam.forward(), cam.right(), cam.up());
    assert!((fwd.length() - 1.0).abs() < 1e-5, "forward единичный");
    assert!((right.length() - 1.0).abs() < 1e-5, "right единичный");
    assert!((up.length() - 1.0).abs() < 1e-5, "up единичный");
    assert!(fwd.dot(right).abs() < 1e-5, "forward ⟂ right");
    assert!(right.dot(up).abs() < 1e-5, "right ⟂ up");
    assert!(fwd.dot(up).abs() < 1e-5, "forward ⟂ up");
    assert!((fwd - (Vec3::ZERO - Vec3::new(1.0, 2.0, 3.0)).normalized()).length() < 1e-5);

    // --- Проверка 7: симметрия при fov = 90 -------------------------------
    let p90 = Mat4::perspective_vk(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
    for r in 0..4 {
        for c in 0..4 {
            let v = p90.at(r, c);
            let want = if r == c { 1.0 } else { 0.0 };
            assert!((v - want).abs() < 1e-5, "fov=90 должна дать единичную матрицу, но at({r},{c}) = {v}");
        }
    }
    println!("  ✓ fov=90 и aspect=1 даёт чистую проекцию на плоскость z");
}

/// ЧАСТЬ 19.5 — ЗАДАНИЕ 19.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Отсечение пирамидой видимости. Каждый кадр это спасает 50-90%
/// объектов, поэтому ошибка тут = «играет, но тормозит» или
/// «иногда объект исчезает».
///
/// ```ignore
/// struct Frustum2 { planes: [Plane; 6] }
/// struct Sphere { c: Vec3, r: f32 }
///
/// impl Frustum2 {
///     fn from_vp(m: &Mat4) -> Self;                    // 6 плоскостей
///     fn visible_sphere(&self, s: &Sphere) -> bool;   // дёшево
///     fn visible_aabb(&self, b: &Aabb) -> bool;      // точнее
/// }
/// ```
///
/// Требования:
///   * Плоскости нормализованы (иначе `r` в p-разделении неверен).
///   * `visible_sphere` — 6 dot-продуктов. `visible_aabb` — 6 формул
///     `dot(c, n) + d + dot(|n|, half) < 0 → не видно`.
///   * Проверь 6 ситуаций: впереди, сзади, слева, справа, сверху,
///     снизу, частично пересекающий near.
///   * `Aabb::from_points(&[Vec3])` — из массива точек, `None` на пустом.
///   * ⚠ ОБЪЯСНИ в комментарии, почему проверка сферы может
///     ОТСЕЧЬ объект, который на самом деле виден, а проверка AABB —
///     нет. Это про консервативность отсечения.
fn task_19_2() {
    struct Sphere {
        c: Vec3,
        r: f32,
    }

    impl Sphere {
        fn new(c: Vec3, r: f32) -> Self {
            not_yet!("assert r > 0");
        }
    }

    struct Frustum2 {
        planes: [Plane; 6],
    }

    impl Frustum2 {
        fn from_vp(m: &Mat4) -> Self {
            // ⚠ ВНИМАНИЕ, ТУТ ЛОВУШКА, И Я НА НЕЙ ПОПАЛСЯ.
            //   Формулы OpenGL: r3±r0 (left/right), r3±r1 (bottom/top),
            //   r3±r2 (near/far). Это верно ТОЛЬКО для clip z в [-1, 1].
            //   В Vulkan z в [0, 1], поэтому near — это просто `r2`,
            //   а far — `r2 - r3`. Копируешь статью про OpenGL —
            //   получаешь объекты, которые исчезают и появляются
            //   при движении камеры, без единой ошибки компилятора.
            not_yet!("Gribb-Hartmann под Vulkan: r3±r0, r3±r1, near=r2, far=r3-r2");
        }
        fn visible_sphere(&self, s: &Sphere) -> bool {
            not_yet!("6 проверок: dot(c, n) + d < -r → не видно");
        }
        fn visible_aabb(&self, b: &Aabb) -> bool {
            not_yet!("то же + dot(|n|, half)");
        }
    }

    let view = Mat4::look_at(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
    let proj = Mat4::perspective_vk(std::f32::consts::FRAC_PI_4, 16.0 / 9.0, 0.5, 100.0);
    let fr = Frustum2::from_vp(&(&view * &proj));

    // Видимые
    assert!(fr.visible_sphere(&Sphere::new(Vec3::new(0.0, 0.0, -10.0), 1.0)));
    assert!(fr.visible_aabb(&Aabb::new(Vec3::new(0.0, 0.0, -10.0), Vec3::new(1.0, 1.0, 1.0))));
    // Частично пересекает near — ОБЯЗАН быть виден
    assert!(fr.visible_sphere(&Sphere::new(Vec3::new(0.0, 0.0, 0.0), 1.0)));
    assert!(fr.visible_aabb(&Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0))));
    // Невидимые
    assert!(!fr.visible_sphere(&Sphere::new(Vec3::new(0.0, 0.0, 10.0), 0.1)));
    assert!(!fr.visible_sphere(&Sphere::new(Vec3::new(1000.0, 0.0, -10.0), 0.1)));
    assert!(!fr.visible_sphere(&Sphere::new(Vec3::new(0.0, 1000.0, -10.0), 0.1)));
    assert!(!fr.visible_sphere(&Sphere::new(Vec3::new(0.0, 0.0, -1000.0), 0.1)));
    assert!(!fr.visible_aabb(&Aabb::new(Vec3::new(0.0, 0.0, 10.0), Vec3::new(1.0, 1.0, 1.0))));

    // ⚠ КОНСЕРВАТИВНОСТЬ: сфера ВПИСАНА в AABB, поэтому
    //   visible_sphere ⊆ visible_aabb. Сфера дешевле, но может
    //   отсечь объект, у которого видна часть AABB.
    //   Именно поэтому в больших сценах делят на «ближние» (сфера)
    //   и «дальние» (AABB): экономия на 90% объектов, точность на 10%.
    let big = Aabb::new(Vec3::new(0.0, 0.0, -10.0), Vec3::new(20.0, 20.0, 0.5));
    let inner = Sphere::new(Vec3::new(0.0, 0.0, -10.0), 0.1);
    assert!(fr.visible_sphere(&inner) && fr.visible_aabb(&big));

    assert!(Aabb::from_points(&[]).is_none());
    let pts = [Vec3::new(-1.0, -2.0, 3.0), Vec3::new(5.0, 4.0, -1.0)];
    let bb = Aabb::from_points(&pts).expect("непустой набор");
    assert_eq!(bb.center, Vec3::new(2.0, 1.0, 1.0));
    assert_eq!(bb.half, Vec3::new(3.0, 3.0, 2.0));
}

/// ЧАСТЬ 19.6 — ЗАДАНИЕ 19.3
/// ─────────────────────────────────────────────────────────────────────────────
/////
/// Конвертация GPU-структур в байты и обратно. Цель: `Roundtrip<A>`
///
/// Тест на равенство байтов после round-trip ловит и смену
/// порядка полей, и забытый `repr(C)`, и ошибку в размере.
///
/// ```ignore
/// #[repr(C)] #[derive(Debug, Clone, Copy, PartialEq)]
/// struct PushConstants { mvp: [[f32; 4]; 4], tint: [f32; 4] }
///
/// fn to_bytes<T>(items: &[T]) -> Vec<u8>;
/// fn from_bytes_exact<T: Copy + Default>(bytes: &[u8]) -> Result<Vec<T>, CastError>;
/// ```
///
/// ⚠ `from_bytes_exact` для ЛЮБОГО T невозможно безопасно: не любое
///   содержимое `u8` — валидный `T`. Например, `bool` принимает
///   только 0 и 1, `NonZeroU32` — только ненулевое. Поэтому делаем:
///
///   1. Проверка размера: `bytes.len() % size_of::<T>() == 0`.
///   2. Проверка выравнивания.
///   3. Копирование через `ptr::read_unaligned` в заранее проверенный
///      буфер — но только если T: Copy (копия байт не создаёт
///      «невалидных» промежуточных значений, потому что исходные
///      байты пришли от того же T).
///   4. Для `bool`-ов и `NonZero` — после чтения проверить и вернуть Err.
///
/// Требования:
///   * `PushConstants` = 64 + 16 = 80 байт, кратно 16.
///   * `to_bytes(&[pc])` даёт 80 байт и совпадает с `size_of`.
///   * `from_bytes_exact` восстанавливает исходный `pc` байт в байт.
///   * Битый вход (длина не кратна, или байты «невалидного bool»)
///     → `Err`, не паника.
///   * Добавь `#[repr(C)] struct WithBool { flag: bool, x: u32 }` и
///     проверь, что `to_bytes` даёт 5 байт, а `from_bytes_exact`
///     отвергает `flag = 2`. ⚠ Это и есть настоящая цена «просто
///     reinterpret».
fn task_19_3() {
    #[derive(Debug, PartialEq)]
    enum CastError {
        BadLen { got: usize, elem: usize },
        BadAlign { addr: usize, need: usize },
        InvalidBool { at: usize, value: u8 },
    }

    impl std::fmt::Display for CastError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            not_yet!("три варианта — три сообщения");
        }
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct PushConstants {
        mvp: [[f32; 4]; 4],
        tint: [f32; 4],
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct WithBool {
        flag: bool,
        x: u32,
    }

    fn to_bytes<T: Copy>(items: &[T]) -> Vec<u8> {
        not_yet!("копия побайтово: from_raw_parts на &[u8] + extend");
    }

    fn from_bytes_exact<T: Copy>(bytes: &[u8]) -> Result<Vec<T>, CastError> {
        not_yet!("проверки длины и выравнивания, затем копия");
    }

    assert_eq!(size_of::<PushConstants>(), 80);
    assert_eq!(size_of::<WithBool>(), 8, "repr(C) выравнивает u32 после bool → 8 байт");

    let identity: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let pc = PushConstants { mvp: identity, tint: [1.0, 0.5, 0.25, 1.0] };
    let b = to_bytes(&[pc]);
    assert_eq!(b.len(), 80);
    let back = from_bytes_exact::<PushConstants>(&b).expect("валидный буфер");
    assert_eq!(to_bytes(&back), b, "байт в байт");
    assert_eq!(back, vec![pc]);

    assert!(matches!(from_bytes_exact::<PushConstants>(&b[..40]), Err(CastError::BadLen { .. })));
    assert!(matches!(from_bytes_exact::<PushConstants>(&b[1..]), Err(CastError::BadAlign { .. })));
}
