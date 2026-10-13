# 🟣 Twitch Channel Points Miner Go

Minerador ultra-leve e de alta performance de **Pontos de Canal da Twitch** desenvolvido em Go.

- **Zero Navegadores:** Não utiliza Chrome, Puppeteer, Selenium nem consome 500 MB+ de RAM.
- **Ultra-Leve:** Consome apenas **~2 MB de RAM** e menos de **0.1% de CPU**.
- **Auto-Baú Instantâneo (+50 pts):** Conecta via WebSocket PubSub oficial da Twitch e clica no baú de bônus em milissegundos assim que ele aparece.
- **Multi-Stream:** Assiste e minera múltiplos canais simultaneamente até o limite configurado (`maxConcurrent`).
- **Dashboard Web:** Painel moderno em tempo real em `http://localhost:3010`.

---

## 🔑 Como Obter seu Auth Token da Twitch

1. Abra o navegador e entre na sua conta na [Twitch](https://www.twitch.tv).
2. Pressione `F12` para abrir as Ferramentas do Desenvolvedor.
3. Vá na aba **Application** (ou **Armazenamento**) -> **Cookies** -> `https://www.twitch.tv`.
4. Procure pelo cookie chamado **`auth-token`**.
5. Copie o valor desse cookie e cole no campo `"authToken"` no seu `config.json`.

---

## 🚀 Como Executar

### No Windows:
1. Copie `config.example.json` para `config.json` e adicione seu token e streamers favoritos.
2. Dê 2 cliques em `iniciar.bat` (ou execute `twitch-miner.exe`).
3. Acesse o painel em: `http://localhost:3010`.

### No Linux / VPS:
```bash
chmod +x twitch-miner-linux
./twitch-miner-linux
```

---

## ⚙️ Configuração (`config.json`)

```json
{
  "accounts": [
    {
      "username": "seu_usuario",
      "authToken": "seu_auth_token_aqui",
      "streamers": [
        "gaules",
        "alanzoka",
        "casimito",
        "loud_coringa"
      ],
      "maxConcurrent": 6
    }
  ],
  "settings": {
    "checkIntervalSeconds": 60,
    "autoClaimBonus": true,
    "webDashboardPort": 3010
  }
}
```
