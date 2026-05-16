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
}
