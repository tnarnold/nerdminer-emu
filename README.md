# NerdMiner Emulator (Rust)

Emulador desktop do [NerdMiner v2](https://github.com/BitMaker-hub/NerdMiner_v2): um minerador
**solo** de Bitcoin ("loteria") com a mesma interface da T-Display (320x170), rodando como um
pequeno "dock" na sua área de trabalho, escondendo na bandeja do sistema e com controle de uso de
CPU e GPU.

Funciona em **macOS, Linux e Windows**.

## Recursos

- As 4 telas originais (Miner, Clock, Global Stats e BTC Price) com os bitmaps, fontes e
  coordenadas do firmware original. Clique na tela para trocar (como o botão do aparelho) e arraste para mover.
- Stratum v1 (public-pool.io, solo.ckpool.org ou qualquer pool solo), com `mining.suggest_difficulty`.
- Mineração em **CPU** com nº de threads e % de uso por thread ajustáveis ao vivo (duty cycle);
  usa as instruções SHA do processador quando existem (ARMv8 / SHA-NI).
- Mineração em **GPU** via compute shader WGSL (wgpu → Metal / Vulkan / DX12) com % de uso ajustável.
- Bandeja do sistema: mostrar/ocultar, pausar/retomar, sair. No macOS o hashrate aparece na barra de menus.
- Janela sem bordas, opção "sempre no topo", zoom 1x–2.5x, iniciar escondido.
- Dados de rede (preço, altura, hashrate global, taxas, halving) do mempool.space.

> Mineração solo com CPU/GPU é uma loteria: a chance de achar um bloco é minúscula.
> O objetivo é diversão e aprendizado, igual ao NerdMiner físico.

## Compilar e rodar

Requer Rust estável (https://rustup.rs).

```sh
cargo run --release
```

### Dependências no Linux

```sh
sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev \
  libxi-dev libgl1-mesa-dev libvulkan-dev libdbus-1-dev pkg-config
```

A bandeja no Linux usa StatusNotifierItem (D-Bus, crate `ksni`). Em GNOME é preciso a extensão
"AppIndicator and KStatusNotifierItem Support"; KDE, XFCE, Cinnamon etc. já suportam.

## Configuração

Na primeira execução o painel ⚙ abre: informe seu **endereço BTC** e clique em
*Salvar e reconectar*. A configuração fica em:

- macOS: `~/Library/Application Support/nerdminer-emu/config.json`
- Linux: `~/.config/nerdminer-emu/config.json`
- Windows: `%APPDATA%\nerdminer-emu\config.json`

Variáveis úteis para desenvolvimento:

- `NERDMINER_CONFIG=/caminho/config.json`: usa outro arquivo de configuração.
- `NERDMINER_SHOTS=/diretório`: salva capturas (RGBA bruto) de cada tela e encerra.

## Testes

```sh
cargo test --release -- --nocapture
```

Inclui o hash do bloco gênese (caminho com midstate) e um teste de GPU que precisa encontrar o
nonce real do bloco gênese.

## Estrutura

| Arquivo | Função |
|---|---|
| `src/work.rs` | header do bloco, coinbase/merkle, SHA-256d com midstate, dificuldade |
| `src/stratum.rs` | cliente Stratum v1 |
| `src/miner.rs` | threads de CPU com limite de uso, cálculo de hashrate |
| `src/gpu.rs`, `src/sha256d.wgsl` | minerador GPU (wgpu) |
| `src/screens.rs` | telas do NerdMiner (bitmaps, fontes, coordenadas) |
| `src/app.rs` | janela, barra de ferramentas e configurações |
| `src/tray.rs` | bandeja do sistema |
| `src/netinfo.rs` | dados da rede via mempool.space |

## Créditos

Interface baseada no NerdMiner v2 (© 2023 Bitmaker, MIT). Veja `assets/CREDITS.md`.
