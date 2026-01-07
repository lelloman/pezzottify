package com.lelloman.simpleaiassistant.`data`.local

import androidx.room.EntityInsertAdapter
import androidx.room.RoomDatabase
import androidx.room.coroutines.createFlow
import androidx.room.util.getColumnIndexOrThrow
import androidx.room.util.performSuspending
import androidx.sqlite.SQLiteStatement
import javax.`annotation`.processing.Generated
import kotlin.Int
import kotlin.Long
import kotlin.String
import kotlin.Suppress
import kotlin.Unit
import kotlin.collections.List
import kotlin.collections.MutableList
import kotlin.collections.mutableListOf
import kotlin.reflect.KClass
import kotlinx.coroutines.flow.Flow

@Generated(value = ["androidx.room.RoomProcessor"])
@Suppress(names = ["UNCHECKED_CAST", "DEPRECATION", "REDUNDANT_PROJECTION", "REMOVAL"])
public class ChatMessageDao_Impl(
  __db: RoomDatabase,
) : ChatMessageDao {
  private val __db: RoomDatabase

  private val __insertAdapterOfChatMessageEntity: EntityInsertAdapter<ChatMessageEntity>
  init {
    this.__db = __db
    this.__insertAdapterOfChatMessageEntity = object : EntityInsertAdapter<ChatMessageEntity>() {
      protected override fun createQuery(): String = "INSERT OR REPLACE INTO `chat_messages` (`id`,`role`,`content`,`toolCallsJson`,`toolCallId`,`toolName`,`timestamp`) VALUES (?,?,?,?,?,?,?)"

      protected override fun bind(statement: SQLiteStatement, entity: ChatMessageEntity) {
        statement.bindText(1, entity.id)
        statement.bindText(2, entity.role)
        statement.bindText(3, entity.content)
        val _tmpToolCallsJson: String? = entity.toolCallsJson
        if (_tmpToolCallsJson == null) {
          statement.bindNull(4)
        } else {
          statement.bindText(4, _tmpToolCallsJson)
        }
        val _tmpToolCallId: String? = entity.toolCallId
        if (_tmpToolCallId == null) {
          statement.bindNull(5)
        } else {
          statement.bindText(5, _tmpToolCallId)
        }
        val _tmpToolName: String? = entity.toolName
        if (_tmpToolName == null) {
          statement.bindNull(6)
        } else {
          statement.bindText(6, _tmpToolName)
        }
        statement.bindLong(7, entity.timestamp)
      }
    }
  }

  public override suspend fun insert(message: ChatMessageEntity): Unit = performSuspending(__db, false, true) { _connection ->
    __insertAdapterOfChatMessageEntity.insert(_connection, message)
  }

  public override suspend fun insertAll(messages: List<ChatMessageEntity>): Unit = performSuspending(__db, false, true) { _connection ->
    __insertAdapterOfChatMessageEntity.insert(_connection, messages)
  }

  public override fun observeAll(): Flow<List<ChatMessageEntity>> {
    val _sql: String = "SELECT * FROM chat_messages ORDER BY timestamp ASC"
    return createFlow(__db, false, arrayOf("chat_messages")) { _connection ->
      val _stmt: SQLiteStatement = _connection.prepare(_sql)
      try {
        val _columnIndexOfId: Int = getColumnIndexOrThrow(_stmt, "id")
        val _columnIndexOfRole: Int = getColumnIndexOrThrow(_stmt, "role")
        val _columnIndexOfContent: Int = getColumnIndexOrThrow(_stmt, "content")
        val _columnIndexOfToolCallsJson: Int = getColumnIndexOrThrow(_stmt, "toolCallsJson")
        val _columnIndexOfToolCallId: Int = getColumnIndexOrThrow(_stmt, "toolCallId")
        val _columnIndexOfToolName: Int = getColumnIndexOrThrow(_stmt, "toolName")
        val _columnIndexOfTimestamp: Int = getColumnIndexOrThrow(_stmt, "timestamp")
        val _result: MutableList<ChatMessageEntity> = mutableListOf()
        while (_stmt.step()) {
          val _item: ChatMessageEntity
          val _tmpId: String
          _tmpId = _stmt.getText(_columnIndexOfId)
          val _tmpRole: String
          _tmpRole = _stmt.getText(_columnIndexOfRole)
          val _tmpContent: String
          _tmpContent = _stmt.getText(_columnIndexOfContent)
          val _tmpToolCallsJson: String?
          if (_stmt.isNull(_columnIndexOfToolCallsJson)) {
            _tmpToolCallsJson = null
          } else {
            _tmpToolCallsJson = _stmt.getText(_columnIndexOfToolCallsJson)
          }
          val _tmpToolCallId: String?
          if (_stmt.isNull(_columnIndexOfToolCallId)) {
            _tmpToolCallId = null
          } else {
            _tmpToolCallId = _stmt.getText(_columnIndexOfToolCallId)
          }
          val _tmpToolName: String?
          if (_stmt.isNull(_columnIndexOfToolName)) {
            _tmpToolName = null
          } else {
            _tmpToolName = _stmt.getText(_columnIndexOfToolName)
          }
          val _tmpTimestamp: Long
          _tmpTimestamp = _stmt.getLong(_columnIndexOfTimestamp)
          _item = ChatMessageEntity(_tmpId,_tmpRole,_tmpContent,_tmpToolCallsJson,_tmpToolCallId,_tmpToolName,_tmpTimestamp)
          _result.add(_item)
        }
        _result
      } finally {
        _stmt.close()
      }
    }
  }

  public override suspend fun getAll(): List<ChatMessageEntity> {
    val _sql: String = "SELECT * FROM chat_messages ORDER BY timestamp ASC"
    return performSuspending(__db, true, false) { _connection ->
      val _stmt: SQLiteStatement = _connection.prepare(_sql)
      try {
        val _columnIndexOfId: Int = getColumnIndexOrThrow(_stmt, "id")
        val _columnIndexOfRole: Int = getColumnIndexOrThrow(_stmt, "role")
        val _columnIndexOfContent: Int = getColumnIndexOrThrow(_stmt, "content")
        val _columnIndexOfToolCallsJson: Int = getColumnIndexOrThrow(_stmt, "toolCallsJson")
        val _columnIndexOfToolCallId: Int = getColumnIndexOrThrow(_stmt, "toolCallId")
        val _columnIndexOfToolName: Int = getColumnIndexOrThrow(_stmt, "toolName")
        val _columnIndexOfTimestamp: Int = getColumnIndexOrThrow(_stmt, "timestamp")
        val _result: MutableList<ChatMessageEntity> = mutableListOf()
        while (_stmt.step()) {
          val _item: ChatMessageEntity
          val _tmpId: String
          _tmpId = _stmt.getText(_columnIndexOfId)
          val _tmpRole: String
          _tmpRole = _stmt.getText(_columnIndexOfRole)
          val _tmpContent: String
          _tmpContent = _stmt.getText(_columnIndexOfContent)
          val _tmpToolCallsJson: String?
          if (_stmt.isNull(_columnIndexOfToolCallsJson)) {
            _tmpToolCallsJson = null
          } else {
            _tmpToolCallsJson = _stmt.getText(_columnIndexOfToolCallsJson)
          }
          val _tmpToolCallId: String?
          if (_stmt.isNull(_columnIndexOfToolCallId)) {
            _tmpToolCallId = null
          } else {
            _tmpToolCallId = _stmt.getText(_columnIndexOfToolCallId)
          }
          val _tmpToolName: String?
          if (_stmt.isNull(_columnIndexOfToolName)) {
            _tmpToolName = null
          } else {
            _tmpToolName = _stmt.getText(_columnIndexOfToolName)
          }
          val _tmpTimestamp: Long
          _tmpTimestamp = _stmt.getLong(_columnIndexOfTimestamp)
          _item = ChatMessageEntity(_tmpId,_tmpRole,_tmpContent,_tmpToolCallsJson,_tmpToolCallId,_tmpToolName,_tmpTimestamp)
          _result.add(_item)
        }
        _result
      } finally {
        _stmt.close()
      }
    }
  }

  public override suspend fun getById(id: String): ChatMessageEntity? {
    val _sql: String = "SELECT * FROM chat_messages WHERE id = ?"
    return performSuspending(__db, true, false) { _connection ->
      val _stmt: SQLiteStatement = _connection.prepare(_sql)
      try {
        var _argIndex: Int = 1
        _stmt.bindText(_argIndex, id)
        val _columnIndexOfId: Int = getColumnIndexOrThrow(_stmt, "id")
        val _columnIndexOfRole: Int = getColumnIndexOrThrow(_stmt, "role")
        val _columnIndexOfContent: Int = getColumnIndexOrThrow(_stmt, "content")
        val _columnIndexOfToolCallsJson: Int = getColumnIndexOrThrow(_stmt, "toolCallsJson")
        val _columnIndexOfToolCallId: Int = getColumnIndexOrThrow(_stmt, "toolCallId")
        val _columnIndexOfToolName: Int = getColumnIndexOrThrow(_stmt, "toolName")
        val _columnIndexOfTimestamp: Int = getColumnIndexOrThrow(_stmt, "timestamp")
        val _result: ChatMessageEntity?
        if (_stmt.step()) {
          val _tmpId: String
          _tmpId = _stmt.getText(_columnIndexOfId)
          val _tmpRole: String
          _tmpRole = _stmt.getText(_columnIndexOfRole)
          val _tmpContent: String
          _tmpContent = _stmt.getText(_columnIndexOfContent)
          val _tmpToolCallsJson: String?
          if (_stmt.isNull(_columnIndexOfToolCallsJson)) {
            _tmpToolCallsJson = null
          } else {
            _tmpToolCallsJson = _stmt.getText(_columnIndexOfToolCallsJson)
          }
          val _tmpToolCallId: String?
          if (_stmt.isNull(_columnIndexOfToolCallId)) {
            _tmpToolCallId = null
          } else {
            _tmpToolCallId = _stmt.getText(_columnIndexOfToolCallId)
          }
          val _tmpToolName: String?
          if (_stmt.isNull(_columnIndexOfToolName)) {
            _tmpToolName = null
          } else {
            _tmpToolName = _stmt.getText(_columnIndexOfToolName)
          }
          val _tmpTimestamp: Long
          _tmpTimestamp = _stmt.getLong(_columnIndexOfTimestamp)
          _result = ChatMessageEntity(_tmpId,_tmpRole,_tmpContent,_tmpToolCallsJson,_tmpToolCallId,_tmpToolName,_tmpTimestamp)
        } else {
          _result = null
        }
        _result
      } finally {
        _stmt.close()
      }
    }
  }

  public override suspend fun count(): Int {
    val _sql: String = "SELECT COUNT(*) FROM chat_messages"
    return performSuspending(__db, true, false) { _connection ->
      val _stmt: SQLiteStatement = _connection.prepare(_sql)
      try {
        val _result: Int
        if (_stmt.step()) {
          val _tmp: Int
          _tmp = _stmt.getLong(0).toInt()
          _result = _tmp
        } else {
          _result = 0
        }
        _result
      } finally {
        _stmt.close()
      }
    }
  }

  public override suspend fun deleteAll() {
    val _sql: String = "DELETE FROM chat_messages"
    return performSuspending(__db, false, true) { _connection ->
      val _stmt: SQLiteStatement = _connection.prepare(_sql)
      try {
        _stmt.step()
      } finally {
        _stmt.close()
      }
    }
  }

  public override suspend fun deleteAfterTimestamp(timestamp: Long) {
    val _sql: String = "DELETE FROM chat_messages WHERE timestamp > ?"
    return performSuspending(__db, false, true) { _connection ->
      val _stmt: SQLiteStatement = _connection.prepare(_sql)
      try {
        var _argIndex: Int = 1
        _stmt.bindLong(_argIndex, timestamp)
        _stmt.step()
      } finally {
        _stmt.close()
      }
    }
  }

  public companion object {
    public fun getRequiredConverters(): List<KClass<*>> = emptyList()
  }
}
