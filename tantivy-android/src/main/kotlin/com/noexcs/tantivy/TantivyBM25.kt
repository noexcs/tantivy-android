package com.noexcs.tantivy

import java.io.Closeable

/**
 * A BM25 keyword relevance scorer backed by Tantivy (Rust) via JNI.
 *
 * Usage:
 * ```kotlin
 * // In-memory (no persistence)
 * TantivyBM25().use { bm25 ->
 *     bm25.rebuildIndex(listOf(Doc("1", "user", "Alice from Beijing")))
 *     val results = bm25.search("Beijing", topK = 5)
 * }
 *
 * // Disk-backed (persists across sessions)
 * TantivyBM25(context.filesDir.resolve("search_index").absolutePath).use { bm25 ->
 *     bm25.addOrUpdate(Doc("42", "note", "会议记录"))
 * }
 * ```
 *
 * Thread-safe: all public methods acquire an internal Mutex before touching
 * the native index. Multiple coroutines may call [search] concurrently.
 */
class TantivyBM25 private constructor(private var nativePtr: Long) : Closeable {

    /** Creates an in-memory index. */
    constructor() : this(nativeCreate())

    /** Creates or opens a disk-backed index at the given directory path. */
    constructor(path: String) : this(nativeOpenOrCreate(path))

    /**
     * Replaces the entire index with the given documents.
     * Clears any previously indexed data.
     */
    fun rebuildIndex(docs: List<Doc>) {
        checkNotClosed()
        nativeRebuildIndex(nativePtr, docs)
    }

    /**
     * Adds or updates a single document. If a document with the same [Doc.id]
     * already exists, it is replaced atomically.
     */
    fun addOrUpdate(doc: Doc) {
        checkNotClosed()
        nativeAddOrUpdate(nativePtr, doc.id, doc.headerKey, doc.text)
    }

    /**
     * Removes all documents whose [Doc.headerKey] matches the given key.
     */
    fun removeByHeader(headerKey: String) {
        checkNotClosed()
        nativeRemoveByHeader(nativePtr, headerKey)
    }

    /**
     * Searches the index with BM25 scoring and returns the top-K results
     * as a map of document ID to relevance score (higher = more relevant).
     */
    fun search(query: String, topK: Int = 5): Map<String, Float> {
        checkNotClosed()
        if (query.isBlank()) return emptyMap()
        return nativeSearch(nativePtr, query, topK)
    }

    override fun close() {
        if (nativePtr != 0L) {
            nativeClose(nativePtr)
            nativePtr = 0
        }
    }

    private fun checkNotClosed() {
        check(nativePtr != 0L) { "TantivyBM25 has been closed" }
    }

    // ─── JNI native methods ───

    private external fun nativeRebuildIndex(ptr: Long, docs: List<Doc>)
    private external fun nativeAddOrUpdate(ptr: Long, id: String, headerKey: String, text: String)
    private external fun nativeRemoveByHeader(ptr: Long, headerKey: String)
    private external fun nativeSearch(ptr: Long, query: String, topK: Int): Map<String, Float>
    private external fun nativeClose(ptr: Long)

    companion object {
        init {
            System.loadLibrary("tantivy_android")
        }

        @JvmStatic private external fun nativeCreate(): Long
        @JvmStatic private external fun nativeOpenOrCreate(path: String): Long
    }
}

/**
 * A document in the TantivyBM25 index.
 */
data class Doc(
    val id: String,
    val headerKey: String,
    val text: String
)
