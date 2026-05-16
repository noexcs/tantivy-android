package com.noexcs.tantivy

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors

@RunWith(AndroidJUnit4::class)
class TantivyBM25Test {

    private var bm25: TantivyBM25? = null

    @After
    fun tearDown() {
        bm25?.close()
    }

    @Test
    fun createAndClose() {
        bm25 = TantivyBM25()
        bm25!!.close()
        bm25 = null
    }

    @Test(expected = IllegalStateException::class)
    fun searchAfterCloseThrows() {
        bm25 = TantivyBM25()
        bm25!!.close()
        bm25!!.search("test")
    }

    @Test(expected = IllegalStateException::class)
    fun addAfterCloseThrows() {
        bm25 = TantivyBM25()
        bm25!!.close()
        bm25!!.addOrUpdate(Doc("1", "k", "text"))
    }

    @Test
    fun rebuildAndSearch() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(
            listOf(
                Doc("1", "user", "Alice from Beijing"),
                Doc("2", "project", "Rust programming language"),
            )
        )
        val results = bm25!!.search("Beijing", topK = 5)
        assertTrue("Should find doc 1", results.containsKey("1"))
    }

    @Test
    fun searchReturnsTopK() {
        bm25 = TantivyBM25()
        val docs = (0..10).map {
            Doc(it.toString(), "t", "document number $it")
        }
        bm25!!.rebuildIndex(docs)
        val results = bm25!!.search("document", topK = 3)
        assertEquals(3, results.size)
    }

    @Test
    fun searchBlankQueryReturnsEmpty() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(listOf(Doc("1", "a", "hello")))
        assertTrue(bm25!!.search("").isEmpty())
        assertTrue(bm25!!.search("  ").isEmpty())
    }

    @Test
    fun addOrUpdate() {
        bm25 = TantivyBM25()
        bm25!!.addOrUpdate(Doc("1", "user", "hello world"))
        val results = bm25!!.search("hello", topK = 5)
        assertTrue(results.containsKey("1"))

        // Update
        bm25!!.addOrUpdate(Doc("1", "user", "goodbye world"))
        val after = bm25!!.search("goodbye", topK = 5)
        assertTrue(after.containsKey("1"))
    }

    @Test
    fun removeByHeader() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(
            listOf(
                Doc("1", "keep", "keep this"),
                Doc("2", "remove", "remove this"),
            )
        )
        bm25!!.removeByHeader("remove")
        val results = bm25!!.search("remove", topK = 5)
        assertTrue(results.isEmpty())
        val kept = bm25!!.search("keep", topK = 5)
        assertTrue(kept.containsKey("1"))
    }

    @Test
    fun concurrentSearch() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(
            listOf(Doc("1", "a", "concurrent test document"))
        )
        val executor = Executors.newFixedThreadPool(4)
        val latch = CountDownLatch(4)
        val errors = mutableListOf<Throwable>()

        repeat(4) {
            executor.submit {
                try {
                    val r = bm25!!.search("concurrent", topK = 5)
                    if (!r.containsKey("1")) {
                        errors.add(AssertionError("Expected doc 1 in results: $r"))
                    }
                } catch (e: Exception) {
                    errors.add(e)
                } finally {
                    latch.countDown()
                }
            }
        }
        latch.await()
        executor.shutdown()
        assertTrue("Concurrent errors: $errors", errors.isEmpty())
    }

    @Test
    fun phraseSearch() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(
            listOf(
                Doc("1", "a", "the quick brown fox"),
                Doc("2", "a", "quick brown fox jumps"),
                Doc("3", "a", "brown fox is quick"),
            )
        )
        // "brown fox" as phrase — should match docs 1, 2, 3
        val results = bm25!!.searchPhrase("brown fox", topK = 5)
        assertTrue(results.containsKey("1"))
        assertTrue(results.containsKey("2"))

        // "quick brown fox" — both docs start/contain this phrase
        val exact = bm25!!.searchPhrase("quick brown fox", topK = 5)
        assertTrue(exact.containsKey("1"))
        assertTrue(exact.containsKey("2"))
    }

    @Test
    fun fuzzySearch() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(
            listOf(Doc("1", "a", "hello world programming"))
        )
        // Typo: "proggraming" vs "programming"
        val results = bm25!!.searchFuzzy("proggraming", distance = 2, topK = 5)
        assertTrue(results.containsKey("1"))
    }

    @Test
    fun searchWithFacets() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(
            listOf(
                Doc("1", "user", "Alice from Beijing"),
                Doc("2", "user", "Bob from Shanghai"),
                Doc("3", "project", "Rust project"),
                Doc("4", "project", "Kotlin project"),
                Doc("5", "project", "Python project"),
                Doc("6", "note", "random note"),
            )
        )
        val result = bm25!!.searchWithFacets("project", topK = 3)
        assertEquals(3, result.results.size)
        assertEquals(3L, result.facets["project"])
        assertNull(result.facets["note"])
    }

    @Test
    fun phraseSearchBlankQuery() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(listOf(Doc("1", "a", "hello")))
        assertTrue(bm25!!.searchPhrase("").isEmpty())
    }

    @Test
    fun fuzzySearchBlankQuery() {
        bm25 = TantivyBM25()
        bm25!!.rebuildIndex(listOf(Doc("1", "a", "hello")))
        assertTrue(bm25!!.searchFuzzy("").isEmpty())
    }
}
