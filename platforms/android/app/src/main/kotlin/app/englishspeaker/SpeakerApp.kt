package app.englishspeaker

import android.app.Application
import app.englishspeaker.data.SpeakerRepository
import app.englishspeaker.data.UiSettings

class SpeakerApp : Application() {
    lateinit var repository: SpeakerRepository
        private set
    lateinit var settings: UiSettings
        private set

    override fun onCreate() {
        super.onCreate()
        instance = this
        settings = UiSettings(this)
        repository = SpeakerRepository(this)
        repository.start()
    }

    companion object {
        lateinit var instance: SpeakerApp
            private set
        val repo: SpeakerRepository get() = instance.repository
        val settings: UiSettings get() = instance.settings
    }
}
