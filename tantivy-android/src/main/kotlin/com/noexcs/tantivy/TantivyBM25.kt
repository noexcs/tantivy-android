package com.noexcs.tantivy

import java.io.Closeable

/**
 * A BM25 keyword relevance scorer backed by Tantivy (Rust) via JNI.
 *
 * Usage:
 * ```kotlin
 * TantivyBM25().use { bm25 ->
 *     bm25.rebuildIndex(listOf(Doc("1", "user", "Alice from Beijing")))
 *     val results = bm25.search("Beijing", topK = 5)
 * }
 * ```
 *
 * Thread-safe: all public methods acquire an internal Mutex before touching
 * the native index. Multiple coroutines may call [search] concurrently.
 */
class TantivyBM25 : Closeable {

    /** Opaque pointer to the Rust NativeIndex (stored as long to avoid GC issues). */
    private var nativePtr: Long = 0

    init {
        nativePtr = nativeCreate()
    }

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

    private external fun nativeCreate(): Long
    private external fun nativeRebuildIndex(ptr: Long, docs: List<Doc>)
    private external fun nativeAddOrUpdate(ptr: Long, id: String, headerKey: String, text: String)
    private external fun nativeRemoveByHeader(ptr: Long, headerKey: String)
    private external fun nativeSearch(ptr: Long, query: String, topK: Int): Map<String, Float>
    private external fun nativeClose(ptr: Long)

    companion object {
        init {
            System.loadLibrary("tantivy_android")
        }
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
