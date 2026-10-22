package app.englishspeaker.data

/** 首次启动预置的示例文章（与 iOS 端相同的原创短文，便于体验与截图）。 */
object SampleArticles {
    data class Sample(val title: String, val body: String)

    val all = listOf(
        Sample(
            "The Quiet Power of Morning Walks",
            """
            Most of us check our phones before we even get out of bed. But what if the first thing you did each day was walk outside for twenty minutes?

            Researchers have found that an early walk can lift your mood, sharpen your focus, and help you sleep better at night. The light in the morning tells your body clock that the day has begun.

            You don't need any special equipment. Just put on a pair of comfortable shoes, leave your headphones at home, and pick a route that feels safe and pleasant. Look at the trees. Listen to the birds. Notice what has changed since yesterday.

            After a week or two, you may find that your best ideas arrive while you are walking. It is an old habit, but it still works.
            """.trimIndent()
        ),
        Sample(
            "How Coffee Travelled the World",
            """
            According to an old story, a goat herder in Ethiopia noticed that his animals were full of energy after eating the red berries of a certain bush. Whether or not that is true, coffee did begin its journey in East Africa.

            By the fifteenth century, it was being grown in Yemen, and coffee houses soon appeared in cities across the Middle East. People met there to talk, play games, and hear the latest news.

            When coffee arrived in Europe, some people called it a bitter invention. But it caught on quickly, and by the end of the seventeenth century, London alone had hundreds of coffee houses.

            Today, it is one of the most traded goods on earth. The next time you pick up a cup, think about the long road it took to get to you.
            """.trimIndent()
        ),
        Sample(
            "Why We Learn Better by Teaching",
            """
            Have you ever noticed that you understand an idea much better after you explain it to someone else? Psychologists call this the protégé effect.

            When we expect to teach, we organize our thoughts more carefully. We look for the main points, we think of examples, and we find the gaps in our own knowledge.

            You can use this at home. After reading an article, try to explain it out loud in your own words, as if a friend were sitting next to you. If you get stuck, go back and read that part again.

            It might feel a little odd at first, but it is one of the fastest ways to make new knowledge your own.
            """.trimIndent()
        ),
    )
}
