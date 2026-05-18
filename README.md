# Tantivy Android

Android 高性能全文检索库，支持中日韩分词、BM25 评分、模糊搜索和分面聚合。

## 为什么选择它

Android 上做全文检索，常见方案都有坑：

| 方案 | 问题 |
|------|------|
| Apache Lucene | 依赖 Java 9+ API，Android 上直接崩溃 |
| Jetpack AppSearch | 不支持 CJK 分词，中文/日文/韩文搜索效果差 |
| 自己写倒排索引 | 缺少可靠的评分算法，边界情况多 |

Tantivy Android 基于 Rust 生态的 [Tantivy](https://github.com/quickwit-oss/tantivy) 引擎（Quickwit 团队维护，性能比 Lucene 快 2-3 倍），通过 JNI 封装为 AAR 库，开箱即用。

```
接入方式: 一行 Gradle 依赖，零配置
包体积:   ~300KB Kotlin + 按 ABI 的 .so (2.7-4.5MB)
性能:     纯内存索引、无 GC 停顿、SIMD 加速
```

## 快速开始

### 1. 添加依赖

通过 **JitPack** 引入（最简单）：

```kotlin
// settings.gradle.kts
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
        maven("https://jitpack.io")
    }
}

// app/build.gradle.kts
dependencies {
    implementation("com.github.noexcs:tantivy-android:v0.2.0")
}
```

### 2. 开始使用

```kotlin
val bm25 = TantivyBM25()

// 添加文档
bm25.rebuildIndex(
    listOf(
        Doc("1", "contacts", "张三，手机号 13800138000，住在北京朝阳区"),
        Doc("2", "notes", "今天下午三点会议室讨论 Q2 规划"),
        Doc("3", "projects", "Rust 项目迁移到 tantivy 引擎"),
    )
)

// 搜索（支持中文）
val results = bm25.search("北京", topK = 5)
// → {"1": 0.86}

// 用完后关闭（或使用 use {} 自动关闭）
bm25.close()
```

## API 参考

### 基础操作

```kotlin
class TantivyBM25 : Closeable {
    // 构造：内存索引（默认）或磁盘索引
    constructor()                    // 内存，App 退出后丢失
    constructor(path: String)        // 磁盘，数据持久化

    // 全量重建索引
    fun rebuildIndex(docs: List<Doc>)

    // 添加或更新单条文档
    fun addOrUpdate(doc: Doc)

    // 按 headerKey 删除
    fun removeByHeader(headerKey: String)

    // BM25 关键词搜索
    fun search(query: String, topK: Int = 5): Map<String, Float>
}
```

### 高级搜索

```kotlin
// 短语搜索 — 精确匹配连续词组
bm25.searchPhrase("北京朝阳区", topK = 5)

// 模糊搜索 — 容忍拼写错误（distance 越大越宽松）
bm25.searchFuzzy("proggraming", distance = 2, topK = 5)

// 分面搜索 — 同时返回各分类下的匹配数量
val result = bm25.searchWithFacets("会议", topK = 5)
result.results  // Map<String, Float> — 搜索结果
result.facets   // Map<String, Long>  — {"notes": 1, "projects": 0}
```

### 数据模型

```kotlin
data class Doc(
    val id: String,        // 文档唯一标识
    val headerKey: String, // 分类标签（用于按类删除、分面聚合）
    val text: String       // 待索引文本
)

data class SearchResult(
    val results: Map<String, Float>, // docId → score（越高越相关）
    val facets: Map<String, Long>    // headerKey → 匹配数量
)
```

### 内存 vs 磁盘

```kotlin
// 内存模式：适合 10 万文档以内，速度最快
val bm25 = TantivyBM25()

// 磁盘模式：适合数据量更大或需要持久化
val path = context.filesDir.resolve("search_index").absolutePath
val bm25 = TantivyBM25(path)
```

## 支持的语言

| 语言 | 支持 | 效果 |
|------|------|------|
| 中文 | ✅ | 双字分词（bigram），无空格连续文本也能搜 |
| 日文 | ✅ | 汉字 + 平假名 + 片假名 |
| 韩文 | ✅ | 谚文音节双字分词 |
| 英文 | ✅ | 按空格分词，标准 BM25 评分 |

## 支持架构

| ABI | 体积 | 覆盖设备 |
|-----|------|----------|
| arm64-v8a | 3.8 MB | 绝大多数 64 位 Android 手机 |
| armeabi-v7a | 2.7 MB | 老旧 32 位设备 |
| x86_64 | 4.5 MB | 模拟器 |

## 限制

- 适合 **10 万文档以内** 的本地搜索场景（如笔记搜索、通讯录搜索、文档检索）
- 不适用于 百万级+ 文档（建议用服务端方案如 Elasticsearch）
- 暂不支持中文分词库（jieba 等），使用双字分词作为替代

## License

MIT
