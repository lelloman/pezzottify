package com.lelloman.simpleaiassistant.`data`.local

import androidx.room.InvalidationTracker
import androidx.room.RoomOpenDelegate
import androidx.room.migration.AutoMigrationSpec
import androidx.room.migration.Migration
import androidx.room.util.TableInfo
import androidx.room.util.TableInfo.Companion.read
import androidx.room.util.dropFtsSyncTriggers
import androidx.sqlite.SQLiteConnection
import androidx.sqlite.execSQL
import javax.`annotation`.processing.Generated
import kotlin.Lazy
import kotlin.String
import kotlin.Suppress
import kotlin.collections.List
import kotlin.collections.Map
import kotlin.collections.MutableList
import kotlin.collections.MutableMap
import kotlin.collections.MutableSet
import kotlin.collections.Set
import kotlin.collections.mutableListOf
import kotlin.collections.mutableMapOf
import kotlin.collections.mutableSetOf
import kotlin.reflect.KClass

@Generated(value = ["androidx.room.RoomProcessor"])
@Suppress(names = ["UNCHECKED_CAST", "DEPRECATION", "REDUNDANT_PROJECTION", "REMOVAL"])
public class ChatDatabase_Impl : ChatDatabase() {
  private val _chatMessageDao: Lazy<ChatMessageDao> = lazy {
    ChatMessageDao_Impl(this)
  }

  protected override fun createOpenDelegate(): RoomOpenDelegate {
    val _openDelegate: RoomOpenDelegate = object : RoomOpenDelegate(1, "164e917492cd3322e62d8671b9dfc091", "b15c2c8d1369e477c2479007bd76f899") {
      public override fun createAllTables(connection: SQLiteConnection) {
        connection.execSQL("CREATE TABLE IF NOT EXISTS `chat_messages` (`id` TEXT NOT NULL, `role` TEXT NOT NULL, `content` TEXT NOT NULL, `toolCallsJson` TEXT, `toolCallId` TEXT, `toolName` TEXT, `timestamp` INTEGER NOT NULL, PRIMARY KEY(`id`))")
        connection.execSQL("CREATE TABLE IF NOT EXISTS room_master_table (id INTEGER PRIMARY KEY,identity_hash TEXT)")
        connection.execSQL("INSERT OR REPLACE INTO room_master_table (id,identity_hash) VALUES(42, '164e917492cd3322e62d8671b9dfc091')")
      }

      public override fun dropAllTables(connection: SQLiteConnection) {
        connection.execSQL("DROP TABLE IF EXISTS `chat_messages`")
      }

      public override fun onCreate(connection: SQLiteConnection) {
      }

      public override fun onOpen(connection: SQLiteConnection) {
        internalInitInvalidationTracker(connection)
      }

      public override fun onPreMigrate(connection: SQLiteConnection) {
        dropFtsSyncTriggers(connection)
      }

      public override fun onPostMigrate(connection: SQLiteConnection) {
      }

      public override fun onValidateSchema(connection: SQLiteConnection): RoomOpenDelegate.ValidationResult {
        val _columnsChatMessages: MutableMap<String, TableInfo.Column> = mutableMapOf()
        _columnsChatMessages.put("id", TableInfo.Column("id", "TEXT", true, 1, null, TableInfo.CREATED_FROM_ENTITY))
        _columnsChatMessages.put("role", TableInfo.Column("role", "TEXT", true, 0, null, TableInfo.CREATED_FROM_ENTITY))
        _columnsChatMessages.put("content", TableInfo.Column("content", "TEXT", true, 0, null, TableInfo.CREATED_FROM_ENTITY))
        _columnsChatMessages.put("toolCallsJson", TableInfo.Column("toolCallsJson", "TEXT", false, 0, null, TableInfo.CREATED_FROM_ENTITY))
        _columnsChatMessages.put("toolCallId", TableInfo.Column("toolCallId", "TEXT", false, 0, null, TableInfo.CREATED_FROM_ENTITY))
        _columnsChatMessages.put("toolName", TableInfo.Column("toolName", "TEXT", false, 0, null, TableInfo.CREATED_FROM_ENTITY))
        _columnsChatMessages.put("timestamp", TableInfo.Column("timestamp", "INTEGER", true, 0, null, TableInfo.CREATED_FROM_ENTITY))
        val _foreignKeysChatMessages: MutableSet<TableInfo.ForeignKey> = mutableSetOf()
        val _indicesChatMessages: MutableSet<TableInfo.Index> = mutableSetOf()
        val _infoChatMessages: TableInfo = TableInfo("chat_messages", _columnsChatMessages, _foreignKeysChatMessages, _indicesChatMessages)
        val _existingChatMessages: TableInfo = read(connection, "chat_messages")
        if (!_infoChatMessages.equals(_existingChatMessages)) {
          return RoomOpenDelegate.ValidationResult(false, """
              |chat_messages(com.lelloman.simpleaiassistant.data.local.ChatMessageEntity).
              | Expected:
              |""".trimMargin() + _infoChatMessages + """
              |
              | Found:
              |""".trimMargin() + _existingChatMessages)
        }
        return RoomOpenDelegate.ValidationResult(true, null)
      }
    }
    return _openDelegate
  }

  protected override fun createInvalidationTracker(): InvalidationTracker {
    val _shadowTablesMap: MutableMap<String, String> = mutableMapOf()
    val _viewTables: MutableMap<String, Set<String>> = mutableMapOf()
    return InvalidationTracker(this, _shadowTablesMap, _viewTables, "chat_messages")
  }

  public override fun clearAllTables() {
    super.performClear(false, "chat_messages")
  }

  protected override fun getRequiredTypeConverterClasses(): Map<KClass<*>, List<KClass<*>>> {
    val _typeConvertersMap: MutableMap<KClass<*>, List<KClass<*>>> = mutableMapOf()
    _typeConvertersMap.put(ChatMessageDao::class, ChatMessageDao_Impl.getRequiredConverters())
    return _typeConvertersMap
  }

  public override fun getRequiredAutoMigrationSpecClasses(): Set<KClass<out AutoMigrationSpec>> {
    val _autoMigrationSpecsSet: MutableSet<KClass<out AutoMigrationSpec>> = mutableSetOf()
    return _autoMigrationSpecsSet
  }

  public override fun createAutoMigrations(autoMigrationSpecs: Map<KClass<out AutoMigrationSpec>, AutoMigrationSpec>): List<Migration> {
    val _autoMigrations: MutableList<Migration> = mutableListOf()
    return _autoMigrations
  }

  public override fun chatMessageDao(): ChatMessageDao = _chatMessageDao.value
}
