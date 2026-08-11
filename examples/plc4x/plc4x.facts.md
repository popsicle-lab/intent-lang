# PLC4X 统一工业通信 API 功能点事实

## Meta
- domain: PLC4X 统一工业通信 API
- domain_abbrev: P4X
- pinned: plc4x@a2e94f392d31b23a9b5d29b651241dcbd6345cfb
- extracted_at: 2026-08-11
- skill_version: extract-facts/0.2.0
- source_repo: https://github.com/apache/plc4x
- scope: plc4j 统一 API 层（api/、spi/drivers/、tools/connection-cache/、tools/event-pump/），不含各协议驱动实现细节
- tools: rg ✓, git clone ✓

## 能力清单
| 能力 | 用户价值 | 主要入口 |
| 建立连接 | 用连接串选定协议/传输并建立 PLC 会话 | `PlcDriverManager.getConnection(url[, auth])` |
| 读取标签 | 批量读取一个或多个 PLC 标签，异步返回 per-tag 结果 | `connection.readRequestBuilder().addTagAddress(...).build().execute()` |
| 写入标签 | 批量写入标签值，build 阶段校验地址与数据 | `connection.writeRequestBuilder().addTagAddress(name, addr, values...).build().execute()` |
| 订阅标签 | 周期性/变位/事件订阅，推送 `PlcSubscriptionEvent` | `connection.subscriptionRequestBuilder()` + `subscribe()` / `registerConsumer()` |
| 浏览与发现 | 已连接会话内浏览标签树；驱动级网络设备发现 | Browse: `connection.browseRequestBuilder()`；Discovery: `driver.discoveryRequestBuilder()` |
| 连接池复用 | 按连接串复用连接、租约、空闲/ping 校验 | `CachedPlcConnectionManager.getBuilder()...build().getConnection(url)` |
| 定时采集 | 触发式批量采集（Event Pump，原 Scraper 继任） | `TagBatch.builder()` + `Trigger`（如 `TimerTrigger`） |

## 术语表
| 术语 | 含义（来自文档/注释） |
| PlcConnection | 与单个 PLC/工业设备的会话抽象 |
| 连接串 | `{protocol}(:{transport})?://{transport-config}(?{params})` 格式 URI |
| PlcResponseCode | per-tag 响应码（OK/NOT_FOUND/INVALID_ADDRESS 等） |
| Browse | 已连接会话内查 tag/metadata 树 |
| Discovery | 在网络上发现设备/服务，无需目标 PLC 连接串 |
| LeasedPlcConnection | 连接池租约包装，`close()` 归还池而非真正断开 |
| TagBatch | Event Pump 中一组待采集 tag + 触发器 + 监听器 |

## 实体与状态
- DeviceSession：字段 phase ∈ {Disconnected, Connected, ConnectionLost}（@a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/api/types/ConnectionStateChangeType.java#L22-L35 事件类型）
- PlcResponseCode：OK, NOT_FOUND, ACCESS_DENIED, INVALID_ADDRESS, INVALID_DATATYPE, INVALID_DATA, INTERNAL_ERROR, REMOTE_BUSY, REMOTE_ERROR, UNSUPPORTED, RESPONSE_PENDING, NOT_READY, OUT_OF_RANGE（@a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/api/types/PlcResponseCode.java#L24-L91）
- ConnectionPool：字段 closed: Bool（@a2e94f3:plc4j/tools/connection-cache/src/main/java/org/apache/plc4x/java/utils/cache/CachedPlcConnectionManager.java#L111）
- TagBatch：字段 closed / started: Bool（@a2e94f3:plc4j/tools/event-pump/src/main/java/org/apache/plc4x/java/tools/eventpump/TagBatch.java#L97-L98）

## 状态流转
| 源态 | 操作 | 次态 | 锚点 |
| Disconnected | EstablishConnection（成功） | Connected | @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L73-L84 |
| Connected | CloseConnection | Disconnected | @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/api/types/ConnectionStateChangeType.java#L25 |
| Connected | ConnectionLost（网络异常） | ConnectionLost | @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/api/types/ConnectionStateChangeType.java#L26 |
| （无） | BootstrapSession | Disconnected | 会话初始态（驱动 connect 前） |

## 操作

### 操作：EstablishConnection
- source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L73-L145
- 职责：解析连接串、选择驱动、创建连接并调用 connect()
- example 候选: url=`api-mock://some-cool-url` → isConnected=true（@a2e94f3:plc4j/api/src/test/java/org/apache/plc4x/java/PlcDriverManagerTest.java#L47-L51 [来自测试代码，未在本环境运行]）

#### 前置检查
- fact_id: F-P4X-BEH-001
  statement: 连接串 URI scheme 无法解析时抛出 PlcConnectionException 且不返回连接
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L137-L144
  evidence: `catch (URISyntaxException e) { throw new PlcConnectionException("Invalid plc4j connection string ...") }`

- fact_id: F-P4X-BEH-002
  statement: 无已注册驱动匹配 protocol code 时抛出 PlcConnectionException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L123-L128
  evidence: `if (driver == null) { throw new PlcConnectionException("Unable to find driver for protocol ...") }`

- fact_id: F-P4X-BEH-003
  statement: 连接串不匹配 URI_PATTERN 时抛出 PlcConnectionException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/DriverBase.java#L159-L163
  evidence: `if (!matcher.matches()) { throw new PlcConnectionException("Connection string doesn't match the format ...") }`

- fact_id: F-P4X-BEH-004
  statement: 驱动无默认 transport 且连接串未指定 transport code 时抛出 PlcConnectionException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/DriverBase.java#L168-L171
  evidence: `if (transportCodeMatch == null && getMetadata().getDefaultTransportCode().isEmpty()) { throw ... }`

- fact_id: F-P4X-BEH-005
  statement: 连接串 protocol code 与当前驱动不符时抛出 PlcConnectionException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/DriverBase.java#L177-L180
  evidence: `if (!protocolCode.equals(getProtocolCode())) { throw new PlcConnectionException("This driver is not suited ...") }`

- fact_id: F-P4X-BEH-006
  statement: transport 不在驱动支持列表且未设 allow-unsupported-transport=true 时抛出 PlcConnectionException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/DriverBase.java#L194-L201
  evidence: `if (!supportedTransportCodes.contains(transportCode)) { throw new PlcConnectionException(...) }`

- fact_id: F-P4X-BEH-007
  statement: transport 未在 TransportManager 注册时抛出 PlcConnectionException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/DriverBase.java#L206-L207
  evidence: `transportManager.getTransport(transportCode).orElseThrow(() -> new PlcConnectionException("Unsupported transport ..."))`

#### 状态效果
- fact_id: F-P4X-BEH-008
  statement: getConnection 成功路径在返回前调用 connection.connect()
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L78-L80
  evidence: `connection = driver.getConnection(url); connection.connect();`

- fact_id: F-P4X-BEH-009
  statement: ServiceLoader 注册时同一 protocol code 出现多个驱动实现则抛出 IllegalStateException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L56-L59
  evidence: `throw new IllegalStateException("Multiple driver implementations available for protocol code ...")`

#### 错误路径
- fact_id: F-P4X-BEH-010
  statement: 上述任一前置检查失败时不进入 connect()，连接对象不返回给调用方
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L73-L84
  evidence: 异常在 try 块内抛出，return 不可达

### 操作：ReadTags
- source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L301-L331
- 职责：构建批量读请求并通过 CompletableFuture 异步执行
- example 候选: tag=`RANDOM/foo:String`, name=`foo` → response.getResponseCode("foo")（@a2e94f3:plc4j/drivers/simulated/src/test/java/org/apache/plc4x/java/simulated/connection/SimulatedConnectionTest.java#L86-L92 [来自测试代码]）

#### 前置检查
- fact_id: F-P4X-BEH-011
  statement: read builder 中重复 tag name 时抛出 PlcRuntimeException 且不构建请求
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/messages/DefaultPlcReadRequest.java#L104-L106
  evidence: `if (tagItems.containsKey(name)) { throw new PlcRuntimeException("Duplicate tag definition ...") }`

- fact_id: F-P4X-BEH-012
  statement: tagHandler 为 null 时 addTagAddress 抛出 NullPointerException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/messages/DefaultPlcReadRequest.java#L103
  evidence: `Objects.requireNonNull(tagHandler, "tagHandler must not be null")`

#### 状态效果
- fact_id: F-P4X-BEH-013
  statement: tag 地址解析失败时在 build 阶段标记该 tag 为 PlcResponseCode.INVALID_ADDRESS 而非抛异常
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/messages/DefaultPlcReadRequest.java#L107-L113
  evidence: `catch (Exception e) { return new DefaultPlcTagErrorItem<>(PlcResponseCode.INVALID_ADDRESS); }`

- fact_id: F-P4X-BEH-014
  statement: 驱动未实现 onRead 时 execute 返回 failedFuture(PlcRuntimeException("Read not supported"))
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L389-L391
  evidence: `return CompletableFuture.failedFuture(new PlcRuntimeException("Read not supported"));`

- fact_id: F-P4X-BEH-015
  statement: 同一 PlcReadRequest 可含多个 tag name，批量读取
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/messages/DefaultPlcReadRequest.java#L89-L115
  evidence: Builder 使用 LinkedHashMap 存多个 tagItems

#### 错误路径
- fact_id: F-P4X-BEH-016
  statement: 响应中 getPlcValue 对非 OK 的 tag 抛出 PlcRuntimeException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/messages/DefaultPlcReadResponse.java#L226-L228
  evidence: 读取响应值时校验 responseCode

### 操作：WriteTags
- source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L306-L338
- 职责：构建批量写请求并异步执行
- example 候选: name=`bar`, addr=`RANDOM/foo:String`, value=`foobar`（@a2e94f3:plc4j/drivers/simulated/src/test/java/org/apache/plc4x/java/simulated/connection/SimulatedConnectionTest.java#L97-L99 [来自测试代码]）

#### 前置检查
- fact_id: F-P4X-BEH-017
  statement: write builder 中重复 tag name 时抛出 PlcRuntimeException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/messages/DefaultPlcWriteRequest.java#L114-L116
  evidence: `throw new PlcRuntimeException("Duplicate tag definition ...")`

- fact_id: F-P4X-BEH-018
  statement: write values 为空或 null 时在 parsePlcValue 抛出 PlcRuntimeException("Expecting at least 1 item")
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/messages/DefaultPlcWriteRequest.java#L149-L152
  evidence: `if ((values == null) || (values.length == 0)) { throw new PlcRuntimeException(...) }`

#### 状态效果
- fact_id: F-P4X-BEH-019
  statement: tag 地址解析失败时 build 标记 INVALID_ADDRESS；值解析失败标记 INVALID_DATA
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/messages/DefaultPlcWriteRequest.java#L117-L128
  evidence: 两个 catch 分别返回 DefaultPlcTagErrorItem

- fact_id: F-P4X-BEH-020
  statement: build 阶段已有错误的 tag 在 Simulated 驱动执行时不写入设备，响应保留 build-time 错误码
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/drivers/simulated/src/main/java/org/apache/plc4x/java/simulated/connection/SimulatedConnection.java#L168-L179
  evidence: `if (requestCode == PlcResponseCode.OK) { device.set(...) } else { tags.put(tagName, requestCode) }`

- fact_id: F-P4X-BEH-021
  statement: 驱动未实现 onWrite 时 execute 返回 failedFuture(PlcRuntimeException("Write not supported"))
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L393-L395
  evidence: 默认 onWrite 实现

### 操作：SubscribeTags
- source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L311-L347
- 职责：建立 CYCLIC / CHANGE_OF_STATE / EVENT 订阅并通过 consumer 接收事件
- example 候选: cyclic=`STATE/foo:String` interval=1s（@a2e94f3:plc4j/drivers/simulated/src/test/java/org/apache/plc4x/java/simulated/connection/SimulatedConnectionTest.java#L111-L115 [来自测试代码]）

#### 前置检查
- fact_id: F-P4X-BEH-022
  statement: 驱动未实现 onSubscribe 时 subscribe 返回 failedFuture(PlcRuntimeException("Subscribe not supported"))
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L397-L399
  evidence: 默认 onSubscribe 实现

#### 状态效果
- fact_id: F-P4X-BEH-023
  statement: PlcSubscriptionType 支持 CYCLIC、CHANGE_OF_STATE、EVENT 三种订阅类型
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/api/types/PlcSubscriptionType.java#L24-L27
  evidence: enum 定义

- fact_id: F-P4X-BEH-024
  statement: registerConsumer 在驱动未实现时抛出 PlcRuntimeException("Register consumer not supported")
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L405-L407
  evidence: 默认 onRegisterConsumer 实现

### 操作：BrowseTags
- source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L297-L359
- 职责：在已连接会话内浏览 tag/metadata 树

#### 前置检查
- fact_id: F-P4X-BEH-025
  statement: 驱动未实现 onBrowse 时 browse 返回 failedFuture(PlcRuntimeException("Browse not supported"))
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L413-L415
  evidence: 默认 onBrowse 实现

### 操作：DiscoverDevices
- source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/DriverBase.java#L119-L125
- 职责：驱动级网络设备发现，无需目标 PLC 连接串

#### 前置检查
- fact_id: F-P4X-BEH-026
  statement: 驱动 canDiscover 为 false 或未实现 PlcDiscoverer 时 discoveryRequestBuilder 回退到 PlcDriver 默认实现
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/DriverBase.java#L119-L125
  evidence: `return PlcDriver.super.discoveryRequestBuilder();`

- fact_id: F-P4X-BEH-027
  statement: PlcDriver 默认 discoveryRequestBuilder 抛出 PlcNotImplementedException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/api/PlcDriver.java#L113-L115
  evidence: 默认接口实现

### 操作：AcquireCachedConnection
- source: @a2e94f3:plc4j/tools/connection-cache/src/main/java/org/apache/plc4x/java/utils/cache/CachedPlcConnectionManager.java#L132-L178
- 职责：按连接串从池中租约连接，close() 归还

#### 前置检查
- fact_id: F-P4X-BEH-028
  statement: CachedPlcConnectionManager 已 close 时 getConnection 抛出 PlcConnectionManagerClosedException
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/connection-cache/src/main/java/org/apache/plc4x/java/utils/cache/CachedPlcConnectionManager.java#L139-L141
  evidence: `if (closed) { throw new PlcConnectionManagerClosedException(); }`

- fact_id: F-P4X-BEH-029
  statement: 租约等待超时（maxWaitTimeMs + grace）时抛出 PlcConnectionException("Error acquiring lease for connection")
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/connection-cache/src/main/java/org/apache/plc4x/java/utils/cache/CachedPlcConnectionManager.java#L174-L177
  evidence: catch TimeoutException → PlcConnectionException

#### 状态效果
- fact_id: F-P4X-BEH-030
  statement: 连接池按 connectionString 为 key 在 ConcurrentHashMap 中复用 ConnectionContainer
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/connection-cache/src/main/java/org/apache/plc4x/java/utils/cache/CachedPlcConnectionManager.java#L106-L162
  evidence: `cachedConnections.computeIfAbsent(connectionString, ...)`

- fact_id: F-P4X-BEH-031
  statement: 默认配置 maxIdle=5min, maxLease=1min, maxWait=30s, pingTimeout=5s
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/connection-cache/src/main/java/org/apache/plc4x/java/utils/cache/CachedPlcConnectionManager.java#L81-L86
  evidence: DEFAULT_* 常量

### 操作：FetchTagBatch
- source: @a2e94f3:plc4j/tools/event-pump/src/main/java/org/apache/plc4x/java/tools/eventpump/TagBatch.java#L437-L467
- 职责：触发器触发时租约连接、批量 PlcReadRequest、回调 listener

#### 前置检查
- fact_id: F-P4X-BEH-032
  statement: TagBatch 已 closed 时 fetchTags 返回 failedFuture(IllegalStateException("Batch is closed"))
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/event-pump/src/main/java/org/apache/plc4x/java/tools/eventpump/TagBatch.java#L438-L441
  evidence: `return CompletableFuture.failedFuture(new IllegalStateException("Batch is closed"));`

- fact_id: F-P4X-BEH-033
  statement: TagBatch 无 tags 时 fetchTags 跳过读取并返回 completedFuture(null)
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/event-pump/src/main/java/org/apache/plc4x/java/tools/eventpump/TagBatch.java#L443-L446
  evidence: `if (tags.isEmpty()) { return CompletableFuture.completedFuture(null); }`

- fact_id: F-P4X-BEH-034
  statement: 连续失败退避窗口内 fetchTags 跳过并返回 completedFuture(null)
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/event-pump/src/main/java/org/apache/plc4x/java/tools/eventpump/TagBatch.java#L448-L453
  evidence: `if (System.currentTimeMillis() < nextAllowedFetchTimeMs) { return CompletableFuture.completedFuture(null); }`

- fact_id: F-P4X-BEH-035
  statement: 上次 fetch 未完成时新 fetch 被跳过并递增 skippedFetchCount
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/event-pump/src/main/java/org/apache/plc4x/java/tools/eventpump/TagBatch.java#L457-L465
  evidence: `if (!fetchInProgress.compareAndSet(false, true)) { skippedFetchCount++; ... }`

#### 状态效果
- fact_id: F-P4X-BEH-036
  statement: tag 集合不变时 TagBatch 缓存 PlcReadRequest 跨轮询复用
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/tools/event-pump/src/main/java/org/apache/plc4x/java/tools/eventpump/TagBatch.java#L100-L110
  evidence: cachedReadRequest 字段及注释

## 全局不变量
- fact_id: F-P4X-BEH-037
  statement: 连接串格式匹配 `{protocol-code}(:{transport-code})?://{transport-config}(?{param-string})?`
  modality: must
  status: confirmed
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/DriverBase.java#L79-L80
  evidence: URI_PATTERN 正则

- fact_id: F-P4X-BEH-038
  statement: ServiceLoader 注册的协议驱动包括 modbus-tcp、modbus-rtu、s7、ads、opcua、eip、bacnet-ip、knxnet-ip 等 24+ 种（以 META-INF/services 为准）
  modality: may
  status: confirmed
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L53-L63
  evidence: ServiceLoader 遍历 PlcDriver 注册

## 疑似问题区
- fact_id: F-P4X-SUS-001
  statement: ConnectionBase.getMetadata() 默认 isRead/Write/Subscribe/BrowseSupported 均返回 true，与驱动实际能力可能不符
  modality: (unknown)
  status: draft
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L243-L264
  evidence: 四个 override 均 `return true`

- fact_id: F-P4X-SUS-002
  statement: PlcConnection javadoc 声明 builder 可抛 PlcUnsupportedOperationException，但 ConnectionBase 始终返回 builder，不支持时在 execute() 才 failedFuture
  modality: (unknown)
  status: draft
  source: @a2e94f3:plc4j/spi/drivers/src/main/java/org/apache/plc4x/java/spi/drivers/ConnectionBase.java#L389-L415
  evidence: 文档与实现行为不一致

- fact_id: F-P4X-SUS-003
  statement: profinet 与 profinet-ng 两模块均注册 protocol code "profinet"，同时 classpath 会触发 duplicate driver IllegalStateException
  modality: must
  status: draft
  source: @a2e94f3:plc4j/api/src/main/java/org/apache/plc4x/java/DefaultPlcDriverManager.java#L56-L59
  evidence: duplicate protocol code 检查

- fact_id: F-P4X-SUS-004
  statement: 官网 scraper 文档仍引用已移除的 plc4j-scraper 模块；代码库中已改为 event-pump
  modality: (unknown)
  status: draft
  source: @a2e94f3:RELEASE_NOTES
  evidence: scraper → event-pump 迁移记录

## 存疑区
- fact_id: F-P4X-UNK-001
  statement: (unknown — needs human input) 各协议驱动对 discovery/browse/subscribe 的支持矩阵需逐驱动确认
  modality: (unknown)
  status: draft
  source: —
  evidence: 统一 API 有入口，能力由驱动 canDiscover/canBrowse/canSubscribe 决定

- fact_id: F-P4X-UNK-002
  statement: (unknown — needs human input) 订阅 builder 中无效地址是否在 build 时抛异常（与 read/write soft-fail 行为是否一致）
  modality: (unknown)
  status: draft
  source: —
  evidence: DefaultPlcSubscriptionRequest 与 DefaultPlcReadRequest 行为差异待逐行核对

## 未覆盖操作
| 操作 | 原因 |
| PingConnection | 统一 API 有 ping()，本次聚焦读写/订阅/池/采集主路径 |
| UnsubscribeTags | 与 SubscribeTags 对称，优先级低 |
| RegisterDriver（运行时） | 驱动仅 ServiceLoader 静态注册，无运行时 API |

## Extraction Checklist
- [x] 已输出能力清单且与 PRD/未来 goal 同名
- [x] 域边界是业务能力，不是源码模块/CLI 子命令
- [x] 每个操作条目三栏每栏至少一条原子事实或 unknown 哨兵
- [x] 粗扫清单中每个操作：有条目，或记入"未覆盖操作"
- [x] 每条行为事实都有锚点；每个 (unknown) 在存疑区有对应条目
- [x] BEH 条目 status: confirmed（用户授权同会话形式化）；SUS/UNK 仍为 draft
- [x] 无观点、无意图推断
- [x] 工具降级已在 Meta 记录
