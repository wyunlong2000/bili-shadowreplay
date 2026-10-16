# Docker 构建与部署命令

本文汇总从源码构建镜像、部署与更新容器的命令。官方镜像的拉取与使用见 [Docker 部署](../../getting-started/installation/docker)。

## 前置条件

宿主机已安装 Docker，命令在仓库根目录执行。

## 构建镜像

```bash
cd /d/IdeaProjects/bili-shadowreplay
docker build -t bili-shadowreplay:local .
```

构建包含前端（Node）、Rust 后端（cargo-chef 分层缓存）和运行时依赖，首次约 6 分钟。

## 部署容器

首次部署与后续更新使用同一组命令：先移除旧容器，再用本地镜像和既有数据卷重新创建。

```bash
docker stop bili-shadowreplay-local
docker rm bili-shadowreplay-local

docker run -d --name bili-shadowreplay-local \
  --restart unless-stopped \
  -p 3000:3000 \
  -v bili-shadowreplay-data:/app/data \
  -v bili-shadowreplay-cache:/app/cache \
  -v bili-shadowreplay-output:/app/output \
  bili-shadowreplay:local
```

## 构建并部署

更新代码后一条命令完成构建与切换：

```bash
cd /d/IdeaProjects/bili-shadowreplay && \
docker build -t bili-shadowreplay:local . && \
docker stop bili-shadowreplay-local && \
docker rm bili-shadowreplay-local && \
docker run -d --name bili-shadowreplay-local --restart unless-stopped -p 3000:3000 \
  -v bili-shadowreplay-data:/app/data \
  -v bili-shadowreplay-cache:/app/cache \
  -v bili-shadowreplay-output:/app/output \
  bili-shadowreplay:local
```

## 验证

```bash
docker ps --filter name=bili-shadowreplay-local
docker logs --tail 50 bili-shadowreplay-local
curl -s -o /dev/null -w "HTTP %{http_code}\n" http://localhost:3000/
```

启动日志首行输出当前版本号，可用于确认部署的是哪一份构建。

## 回滚

部署前为当前镜像打备份标签：

```bash
docker tag bili-shadowreplay:local bili-shadowreplay:backup-$(date +%Y%m%d)
```

回滚时把[部署容器](#部署容器)中的镜像名换成备份标签即可。

## 数据卷

容器使用三个命名卷保存运行数据，重建容器不会丢失：

| 卷名 | 容器路径 | 内容 |
| --- | --- | --- |
| `bili-shadowreplay-data` | `/app/data` | 数据库与配置 |
| `bili-shadowreplay-cache` | `/app/cache` | 录制缓存 |
| `bili-shadowreplay-output` | `/app/output` | 切片产物 |

使用本地 Whisper 字幕时额外挂载模型卷：

```bash
-v bili-shadowreplay-whisper:/app/whisper_model.bin
```

## 常用运维

```bash
docker logs -f bili-shadowreplay-local
docker exec -it bili-shadowreplay-local sh
docker stats bili-shadowreplay-local
docker inspect bili-shadowreplay-local --format '{{range .Mounts}}{{.Source}} -> {{.Destination}}{{println}}{{end}}'
```
