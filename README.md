# Tantivy-Android

Android 可用的高性能全文检索库，基于 Tantivy (Rust) 通过 JNI 封装，提供原生 BM25 评分、CJK 分词和内存索引。

## 为什么需要这个项目

Android 生态中缺少一个轻量、支持中文、高性能的全文检索库：

| 方案 | 问题 |
|------|------|
| Apache Lucene 9.x | `Runtime.version()` — Android 不含此 Java 9+ API，直接崩溃 |
| Apache Lucene 8.x | `ClassValue` — Android API 34+ 才有，minSdk 33 不可用 |
| Smile NLP | 拉入 5+ MB 传递依赖，无 CJK 分词 |
| Jetpack AppSearch | 面向结构化数据，无 CJK 分词 |
| 手写 BM25 | 功能有限，无生产级分词器 |

**Tantivy** 是 Rust 生态中对标 Lucene 的全文检索引擎，Quickwit 团队维护，性能比 Lucene 快 2-3 倍（无 GC、SIMD 加速、内存紧凑）。本项目通过 JNI 将其封装为 Android AAR 库。

## 核心特性

- **BM25 评分** — Tantivy 原生实现，生产验证
- **CJK 分词** — `tantivy-tokenizer-cjk` 中日韩文字双字分词
- **纯内存索引** — `RamDirectory`，零磁盘 I/O，适合 <10 万文档
- **增量更新** — 单文档增删，无需全量重建
- **零依赖 JNI** — 预编译 `.so` 随 AAR 分发，接入方无需 Rust 工具链

## 项目结构

```
tantivy-android/
├── rust/                                # Rust + JNI 层
│   ├── Cargo.toml                       # Rust 依赖声明
│   └── src/
│       ├── lib.rs                       # JNI 函数导出 (#[no_mangle] pub extern "system")
│       ├── index.rs                     # Tantivy 索引管理 (增删改查)
│       └── tokenizer.rs                 # CJK 分词器注册
├── tantivy-android/                     # Android AAR 模块
│   ├── build.gradle.kts                 # Android 插件 + jniLibs 路径
│   └── src/main/
│       ├── jniLibs/                     # cargo-ndk 编译产物 (预置占位)
│       │   ├── arm64-v8a/libtantivy_android.so
│       │   └── x86_64/libtantivy_android.so
│       └── kotlin/com/noexcs/tantivy/
│           └── TantivyBM25.kt           # 对外公开的 Kotlin API
├── build.gradle.kts                     # Android 根构建
├── settings.gradle.kts                  # 模块声明
├── build_android.sh                     # 一键交叉编译脚本 (cargo-ndk)
├── .github/workflows/                    # CI/CD 自动构建
│   └── build.yml
└── README.md
```

## 快速开始

### 1. 编译 Rust JNI 层

前置依赖：
```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 Android NDK targets
rustup target add aarch64-linux-android x86_64-linux-android

# 安装 cargo-ndk
cargo install cargo-ndk
```

编译：
```bash
./build_android.sh
```

产物输出到 `tantivy-android/src/main/jniLibs/arm64-v8a/libtantivy_android.so` 和 `x86_64/`。

### 2. 集成到 Android 项目

在 `settings.gradle.kts` 中引入模块（或通过 Maven 依赖）：
```kotlin
include(":tantivy-android")
project(":tantivy-android").projectDir = File("../tantivy-android/tantivy-android")
```

在 `app/build.gradle.kts` 中：
```kotlin
implementation(project(":tantivy-android"))
```

### 3. 使用

```kotlin
val bm25 = TantivyBM25()

// 全量建索引
bm25.rebuildIndex(listOf(
    Doc("1", "user", "我叫 Alice，住在北京"),
    Doc("2", "project", "PyMuPDF 用于 PDF 解析"),
))

// 增量更新
bm25.addOrUpdate(Doc("3", "note", "会议时间定在周五下午三点"))

// 删除
bm25.removeByHeader("user")

// 搜索，返回 Map<docId, score>
val results = bm25.search("PDF library", topK = 5)
// → {"2": 2.34}
```

## Kotlin API

```kotlin
data class Doc(val id: String, val headerKey: String, val text: String)

class TantivyBM25 : Closeable {
    fun rebuildIndex(docs: List<Doc>)
    fun addOrUpdate(doc: Doc)
    fun removeByHeader(headerKey: String)
    fun search(query: String, topK: Int = 5): Map<String, Float>
    fun close()
}
```

## Rust JNI 接口

JNI 方法命名：`Java_com_noexcs_tantivy_TantivyBM25_<method>`

| JNI 方法 | 功能 | 关键 Tantivy API |
|----------|------|-----------------|
| `nativeCreate` | 创建 `Index` + `RamDirectory` + CJK tokenizer | `Index::create_in_ram()`, `TextAnalyzer::from(tokenizer)` |
| `nativeRebuildIndex` | 清空并全量写入 | `writer.delete_all_documents()`, `writer.add_document()` |
| `nativeAddOrUpdate` | 单文档更新 | `writer.delete_term(Term::from_field_text(...))`, `writer.add_document()` |
| `nativeRemoveByHeader` | 按 headerKey 删除 | `writer.delete_term(Term::from_field_text("headerKey", key))` |
| `nativeSearch` | BM25 搜索 | `searcher.search(&query, &TopDocs::with_limit(k))` |
| `nativeClose` | 释放资源 | `drop(index)`, `drop(writer)` |

## Tantivy Schema

```rust
// 三个字段
id:        STRING (STORED, INDEXED as raw)
headerKey: STRING (STORED, INDEXED as raw)  
text:      STRING (STORED, INDEXED with CJK tokenizer)
```

## 依赖版本

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tantivy` | 0.22 | 全文检索引擎 |
| CJK 分词器 | 内置 | CJK 双字分词 (自实现，零依赖) |
| `jni` | 0.21 | Rust ↔ JVM 互操作 |
| Android NDK | 27+ | 交叉编译 |
| Kotlin | 2.0+ | AAR 封装 |
| AGP | 8.x | Android 构建 |

## Maven 依赖

```kotlin
// settings.gradle.kts
dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}

// app/build.gradle.kts
dependencies {
    implementation("com.noexcs:tantivy-android:0.1.0")
}
```

## CI/CD

GitHub Actions：PR/push 触发 Rust 测试，tag (`v*`) 触发 `build_android.sh` + AAR 构建，产物上传为 Release Asset。

## 限制与 TODO

- [ ] 仅支持内存索引 (`RamDirectory`)，关闭 App 索引丢失（持久化由上层负责）
- [ ] 仅 arm64-v8a + x86_64，如需 arm-v7a 在 `build_android.sh` 中增加 target
- [ ] 未暴露 Tantivy 高级特性（facet、phrase query、fuzzy search）— 按需添加

## License

同 Tantivy — MIT
