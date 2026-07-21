# PBC Binary Chart Format

`phire/src/bin.rs` 定义 Phire 的 **PBC (Phi Binary Chart)** 二进制谱面格式，由 `prpr-pbc` CLI 工具使用。

## 相关文件

| 文件 | 作用 |
|------|------|
| `phire/src/bin.rs` | 序列化/反序列化核心 (571 行) |
| `prpr-pbc/src/main.rs` | CLI 转换工具 (100 行) |
| `prpr-pbc/Cargo.toml` | 包清单 |

## CLI 用法

```bash
prpr-pbc <输入文件> <输出文件>
```

自动检测输入格式：
- 合法 UTF-8，以 `{` 开头且含 `"META"` → **RPE** (Re:PhiEdit JSON)
- 合法 UTF-8，以 `{` 开头不含 `"META"` → **Pgr** (Phigros JSON)
- 合法 UTF-8，不以 `{` 开头 → **Pec** (.pec 文本谱面)
- 非法 UTF-8 → **Pbc** (已是二进制，可重新序列化)

## 格式总览

- **无魔数/文件头**：通过"非合法 UTF-8"来判断
- **时间编码**：所有时间戳使用毫秒分辨率 **ULEB128 增量编码**
- **字节序**：所有多字节数字类型使用**小端序**

### 顶层结构

```
Chart:
  f64: offset
  uleb: 判定线数量
  每条判定线: JudgeLine
  ChartSettings:
    bool: pe_alpha_extension
    bool: line_reference_y_axis
```

读取后调用 `process_lines(&mut lines)` 做后处理。反序列化时，以下字段设为默认值：
- `bpm_list` = `[(0.0, 60.0)]` — BPM 数据**不保存**在 PBC 中
- `extra` = `ChartExtra::default()` — 特效/视频**不保存**
- `hitsounds` = 空哈希表
- `fonts` = 空

### JudgeLine 编码

```
JudgeLine:
  重置时间累加器
  Object (alpha, scale, rotation, translation)
  Anim<Color>: 颜色
  u8: 类型判别
    0 → Normal
    1 → Texture(_, String)    -- 后跟纹理路径字符串
    2 → Text(Anim<TextData>)  -- 后跟 Anim<TextData>
    3 → Paint(Anim<f32>, _)   -- 后跟 Anim<f32>
  Anim<f64>: 高度
  uleb + Note[]: 音符数组
  Option<usize>: 父级
  bool: rotate_with_parent
  [f32; 2]: 锚点
  bool: show_below
  u8: attach_ui (0=None, 1-7=UIElement 变体)
  CtrlObject
  Anim<f32>: incline
  i32: z_index
```

- `JudgeLineKind::TextureGif` **不支持**二进制序列化（写入时 panic）
- 读取音符后调用 `JudgeLineCache::new` 排序并构建缓存

### Note 编码

```
Note:
  Object (alpha, scale, rotation, translation)
  u8: 种类判别
    0 → Click
    1 → Hold { end_time: f64, end_height: f64, end_speed: Option<f64> }
    2 → Flick
    3 → Drag
  uleb delta: 时间 (毫秒) → 秒
  f64: height
  bool + f64: speed (false=默认1.0, true=显式值)
  bool: above
  bool: fake
  f64: judge_scale
  Anim<Color>: color
  Anim<Color>: hit_fx_color
```

反序列化时设为默认值的字段：
- `hitsound` = `HitSound::default_from_kind(&kind)`
- `multiple_hint` = false
- `judge` = NotJudged
- `protected` = false

### CtrlObject 编码

```
CtrlObject:
  u8: 标记 (必须为 8)
  Anim<f32>: alpha
  Anim<f32>: size
  Anim<f32>: pos
  Anim<f32>: y
```

### Object 编码

```
Object:
  Anim<f32>:    alpha
  Anim<[f32;2]>: scale      (x, y)
  Anim<f32>:    rotation
  Anim<[f32;2]>: translation (x, y)
```

### Anim 编码

`Anim<T>` 是一个段链表。每个段：

```
u8: 判别
  0 → 链结束
  1 → 空关键帧 (Anim::default())
  2 → 有关键帧: uleb 数量，然后该数量的 Keyframe<T>
后跟下一段（重复判别字节）
```

每个段开始时时间累加器归零。

### Keyframe 编码

```
Keyframe<T>:
  uleb delta: 时间 (毫秒) → f64 秒
  value: T
  u8: 缓动判别:
    bits 0xC0 == 0x00 → StaticTween: 字节本身是缓动 ID (0-63)
    bits 0xC0 == 0x80 → ClampedTween: 低 7 位是缓动 ID, 然后 f32 start, f32 end
    bits 0xC0 == 0xC0 → BezierTween: 后跟 4 个 f32 (p1.x, p1.y, p2.x, p2.y)
```

缓动 ID (0-63) 对应 `phire/src/core/tween.rs` 中的缓动函数。

## 基础编码规则

### ULEB128

无符号小端 Base-128 变长整数（标准 protobuf varint 格式）。每个字节 7 位数据，最高位为连续标志 (0x80)。

### 基本类型

| 类型 | 编码 |
|------|------|
| `u8` | 1 字节原始值 |
| `bool` | 1 字节: 0=false, 1=true |
| `i32` | 4 字节小端 |
| `u32` | 4 字节小端 |
| `u64` / `usize` | 8 字节小端 |
| `f32` | 4 字节小端 IEEE 754 |
| `f64` | 8 字节小端 IEEE 754 |
| `[f32; 2]` | 两个连续 f32 |
| `String` | ULEB128 长度 + UTF-8 字节 |
| `Color` | 4 字节 rgba (每通道 0-255) |
| `TextData` | String (text) + `Option<usize>` (font_id) |
| `Option<T>` | 1 字节 bool，true 时后跟 T |
| `Vec<T>` | ULEB128 数量 + 该数量个 T |

### 时间累加器

`BinaryReader` 和 `BinaryWriter` 各维护一个 `u32` 时间累加器。每次读写时间时：

- **写入**：`delta = (v * 1000).round() - accumulator; write ULEB(delta); accumulator = v`
- **读取**：`accumulator += delta; return accumulator / 1000.0`

`reset_time()` 可将累加器归零（每个 Anim 和每个 JudgeLine 开始时调用）。

## 限制

1. **无 BPM 数据** — 总被反序列化为 `(0.0, 60.0)`
2. **无打击音效、特效、视频、字体** — 这些从其他文件加载
3. **不支持 GIF 纹理** — `TextureGif` 在写入时 panic
4. **无版本号** — 格式无版本标记
5. **无测试夹具** — 仓库中无 .pbc 示例文件
