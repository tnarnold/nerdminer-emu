# NerdMiner Emulator (Rust)

[English](README.md) | **Português (BR)**

Emulador desktop do [NerdMiner v2](https://github.com/BitMaker-hub/NerdMiner_v2): um minerador
**solo** de Bitcoin ("loteria") com a mesma interface da T-Display (320x170), rodando como um
pequeno "dock" na sua área de trabalho, escondendo na bandeja do sistema e com controle de uso de
CPU e GPU.

Funciona em **macOS, Linux e Windows**.

🌐 **[Site](https://tnarnold.github.io/nerdminer-emu/)** · 📖 **[Manual do usuário](docs/MANUAL.pt-BR.md)**: configuração, cada tela e opção, como funciona e solução de problemas.

## Recursos

- As 4 telas originais (Miner, Clock, Global Stats e BTC Price) com os bitmaps, fontes e
  coordenadas do firmware original. Clique na tela para trocar (como o botão do aparelho) e arraste para mover.
- Stratum v1 (public-pool.io, solo.ckpool.org ou qualquer pool solo), com `mining.suggest_difficulty`.
- **Seleção do dispositivo de mineração: CPU, GPU ou CPU + GPU**, cada um com seu próprio limite
  de uso, ajustável ao vivo sem reiniciar:
  - **CPU**: nº de threads + % de uso por thread (duty cycle). Usa as instruções SHA do
    processador quando existem (ARMv8 / SHA-NI).
  - **GPU**: % de uso (duty cycle) de um compute shader WGSL (wgpu → Metal / Vulkan / DX12).
    A GPU vem **desligada** por padrão e só é usada se for selecionada.
- Bandeja do sistema como mini painel: bolinha de estado, estatísticas da mineração, **ocupação da rede e a taxa para usar agora** (com o custo em sats e na sua moeda) e controles rápidos de CPU/GPU. No macOS os campos escolhidos aparecem na barra de menus.
- Janela sem bordas, opção "sempre no topo", zoom 1x–2.5x, iniciar escondido.
- Logotipo Go BTC discreto no canto superior esquerdo; clicar abre https://gobtc.com.br.
- Preço do BTC na sua moeda (BRL, USD, EUR, GBP, CAD, CHF, AUD, JPY).
- Dados de rede (preço, altura, hashrate global, taxas, halving) do mempool.space.

> Mineração solo com CPU/GPU é uma loteria: a chance de achar um bloco é minúscula.
> O objetivo é diversão e aprendizado, igual ao NerdMiner físico.

## Consumo de recursos

Medido em um MacBook Air M1 (8 núcleos), build `--release`, minerando contra uma pool local.
CPU do processo no formato do `top` (100% = 1 núcleo inteiro; 800% = a máquina toda).

| Seleção | CPU do processo | % da CPU total | Memória (phys_footprint) |
|---|---:|---:|---:|
| CPU, 4 threads @ 50% (padrão) | ~180% | ~22% | ~40 MB |
| CPU, 4 threads @ 100% | ~393% | ~49% | ~41 MB |
| CPU, 8 threads @ 25% | ~150% | ~19% | ~40 MB |
| CPU, 1 thread @ 10% | ~9% | ~1% | ~40 MB |
| GPU @ 30% | ~2% | <1% | ~41 MB |
| GPU @ 100% | ~5% | <1% | ~42 MB |
| CPU 4 @ 50% + GPU @ 30% | ~179% | ~22% | ~43 MB |

- **Memória**: fica entre ~28 e ~45 MB e se mantém estável ao longo do tempo (sem crescimento).
  A interface (egui) e as 5 telas em textura respondem pela maior parte; o minerador em si usa
  poucos KB por thread. Com a GPU ativa somam-se ~10 MB de buffers/driver.
- **CPU**: o uso segue `threads × % por thread`. Com a GPU selecionada sozinha, a CPU fica quase
  ociosa (só coordena e verifica candidatos).
- **GPU**: o % controla quanto tempo a GPU fica ocupada (cada dispatch dura ~30 ms, seguido de uma
  pausa proporcional), então a interface do sistema continua fluida.
- Hashrate de referência no M1: ~6,5 MH/s por thread de CPU a 100% (4 threads ≈ 26 MH/s) e ~53 MH/s na GPU a 100%.

## Instalar

Baixe o instalador da sua plataforma na página [Releases](https://github.com/tnarnold/nerdminer-emu/releases):

- **macOS** (Apple Silicon e Intel): `NerdMiner-Emulator-<versão>-macos-universal.dmg`. Abra o
  `.dmg` e arraste **NerdMiner Emulator** para **Aplicativos**. Como o app não é notarizado pela
  Apple, na primeira vez clique com o botão direito no app → **Abrir** → **Abrir** (ou rode
  `xattr -dr com.apple.quarantine "/Applications/NerdMiner Emulator.app"`).
- **Windows**: `...-windows-x86_64.zip`, extraia e rode `nerdminer-emu.exe`.
- **Linux**: `...-linux-x86_64.tar.gz`, extraia e rode `./nerdminer-emu`.

### Gerar o .dmg localmente

```sh
scripts/package-macos.sh   # -> dist/NerdMiner-Emulator-<versão>-macos-universal.dmg
```

Com uma conta Apple Developer dá para assinar e notarizar definindo `MACOS_SIGN_IDENTITY` e
`MACOS_NOTARY_PROFILE` (veja o início do script).

### Publicar uma versão

```sh
git tag v0.1.0 && git push origin v0.1.0
```

O workflow `release` gera o `.dmg`, o `.zip` e o `.tar.gz` e cria o Release automaticamente.

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

Todas as opções estão no [manual do usuário](docs/MANUAL.pt-BR.md#5-configurações-). Resumindo: na primeira execução o painel ⚙ abre: informe seu **endereço BTC** e clique em
*Salvar e reconectar*. A configuração fica em:

- macOS: `~/Library/Application Support/nerdminer-emu/config.json`
- Linux: `~/.config/nerdminer-emu/config.json`
- Windows: `%APPDATA%\nerdminer-emu\config.json`

Variáveis úteis para desenvolvimento:

- `NERDMINER_CONFIG=/caminho/config.json`: usa outro arquivo de configuração.
- `NERDMINER_SHOTS=/diretório`: salva capturas (RGBA bruto) de cada tela e encerra.
- `NERDMINER_TRAY_DUMP=1`: imprime o menu da bandeja (título, dica e cada linha) no stderr a cada atualização.

### A pool não conecta

O minerador só faz conexões de **saída** para a pool (como um navegador), então **não precisa de
UPnP nem de redirecionamento de portas**. Se o ponto ao lado do ícone de wifi na tela Miner fica
vermelho, a rede provavelmente bloqueia a porta de saída. Teste com:

```sh
nc -vz public-pool.io 21496     # solo.ckpool.org usa a porta 3333
```

Se der timeout, libere TCP de saída para essa porta no firewall ou teste em outra rede (ex.: hotspot
do celular). O motivo detalhado aparece no log do painel ⚙.

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
