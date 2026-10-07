package com.tabula.app.data

import androidx.room.*
import kotlinx.coroutines.flow.Flow

@Entity(tableName = "saved_matches")
data class SavedMatchEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val gameId: String,
    val gameTitle: String,
    val opponent: String,
    val result: String,
    val ratingChange: String,
    val moveCount: Int,
    val durationText: String,
    val timestamp: Long = System.currentTimeMillis()
)

@Dao
interface SavedMatchDao {
    @Query("SELECT * FROM saved_matches ORDER BY timestamp DESC")
    fun getAllMatches(): Flow<List<SavedMatchEntity>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertMatch(match: SavedMatchEntity): Long

    @Query("DELETE FROM saved_matches WHERE id = :id")
    suspend fun deleteMatchById(id: Long)
}

@Database(entities = [SavedMatchEntity::class], version = 1, exportSchema = false)
abstract class TabulaDatabase : RoomDatabase() {
    abstract fun savedMatchDao(): SavedMatchDao
}
