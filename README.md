# USTChat Cli

USTC Chat API 的 OpenAI 兼容代理。将 USTC Chat 封装为标准的 `/v1/models` 和 `/v1/chat/completions` 接口，让任何 OpenAI 生态工具（ChatBox、Open WebUI、Continue 等）开箱即用。

## 特性

- **OpenAI 兼容** — `/v1/models` 和 `/v1/chat/completions`，支持流式与非流式
- **自动 CAS 登录** — 内置 USTC 统一身份认证
- **可选本地鉴权** — 通过 `--auth` 设置 API Key 保护代理
- **跨平台** — Linux、Windows、macOS（x86_64 / ARM64）预编译二进制

## 快速开始

### 从源码构建

```bash
cargo build --release
```

### 从 CI 下载

在 [Actions](https://github.com/USTC-XeF2/ustchat/actions) 中选择最新成功的 workflow run，下载对应平台的 artifact。

## 用法

```bash
# 基本启动
ustchat run --username <USTC学号> --password <密码>

# 指定端口和监听地址
ustchat run --username SA12345678 --password xxx --port 8080 --host 0.0.0.0

# 启用本地 API Key 鉴权
ustchat run --username SA12345678 --password xxx --auth sk-secret-key

# 使用环境变量（推荐，避免密码出现在 shell 历史）
export USTCHAT_USERNAME=SA12345678
export USTCHAT_PASSWORD=xxx
ustchat run --auth sk-secret-key
```

### CLI 选项

| 选项 | 环境变量 | 默认值 | 说明 |
|---|---|---|---|
| `--username` | `USTCHAT_USERNAME` | — | USTC CAS 用户名（学工号） |
| `--password` | `USTCHAT_PASSWORD` | — | USTC CAS 密码 |
| `--port` | — | `28080` | 代理监听端口 |
| `--host` | — | `127.0.0.1` | 代理监听地址 |
| `--endpoint` | — | `https://chat.ustc.edu.cn` | 上游 API 地址 |
| `--auth` | — | 无 | 客户端需携带的 API Key，可多次指定 |

## API 端点

### `GET /v1/models`

返回可用模型列表。

```bash
curl http://127.0.0.1:28080/v1/models
```

### `POST /v1/chat/completions`

发送聊天补全请求，支持流式与非流式。

```bash
# 非流式
curl http://127.0.0.1:28080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-secret-key" \
  -d '{
    "model": "deepseek-v4-flash",
    "messages": [{"role": "user", "content": "你好"}],
    "stream": false
  }'

# 流式（SSE）
curl http://127.0.0.1:28080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-secret-key" \
  -d '{
    "model": "deepseek-v4-pro",
    "messages": [{"role": "user", "content": "介绍一下中国科学技术大学"}],
    "stream": true
  }'
```

### 可用模型

| ID | 名称 | 推理 | 工具调用 |
|---|---|---|---|
| `deepseek-v4-flash` | USTC Deepseek v4 Flash | 否 | 是 |
| `deepseek-v4-pro` | USTC Deepseek v4 Pro | 否 | 是 |

## 许可

MIT License
