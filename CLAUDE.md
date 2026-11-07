# NerdMiner Emulator

Emulador desktop em Rust do NerdMiner: minerador solo de Bitcoin no estilo loteria, com as telas do T-Display, bandeja do sistema e limites de CPU/GPU. Roda em macOS, Linux e Windows.

- Repositório público: https://github.com/tnarnold/nerdminer-emu (conta `gh`: `tnarnold`)
- Site: https://tnarnold.github.io/nerdminer-emu/
- Trabalho planejado: [`docs/ROADMAP.md`](docs/ROADMAP.md). Comece por ali.
- Manuais do usuário: `docs/MANUAL.md` (en) e `docs/MANUAL.pt-BR.md`.

Fale com o usuário em português do Brasil. Comentários de código e identificadores ficam em inglês; mensagens de commit e o roadmap, em português.

## Regras

- **Dados pessoais fora do repositório.** O repositório é público. O endereço BTC do usuário fica só na config local. Ao ler essa config, filtre apenas as chaves de mineração, sem imprimir o endereço.
- **Chaves de API** nunca entram em commit, nem em arquivo do projeto.
- **Commit e push só quando o usuário pedir.** Push em `main` publica o site se `site/` mudou.
- **Mensagem de commit** termina com o `Co-Authored-By` indicado pelo sistema.
- **Funções opcionais não ocupam memória até serem ativadas.** O app fica o dia todo na bandeja. Guarde o estado num `Option`, crie no clique e solte ao fechar. Sem dependências novas sem necessidade.
- **Visual do laboratório aprovado:** peças de máquina em cerâmica, grafite e aço, sombras suaves, um único destaque laranja, rótulos finos. Nada de cubos coloridos.
- **Números publicados são medidos ou marcados como estimativa.** Não prometa hashrate nem consumo. Hash a hash, a chance é fixa: não sugira que algo "aumenta a chance". Veja "Eficiência de mineração" no roadmap antes de propor otimização.
- **Todo texto de interface existe em pt e en.** Falta de string é erro de compilação em `i18n::Tr` e `lab::text`. O site é bilíngue via `data-l="en"` e `data-l="pt"`. Mudança visível atualiza os dois manuais e os dois READMEs.

## Comandos

`source ~/.cargo/env` antes de usar o cargo. O shell é zsh; `timeout` não existe no macOS (use `perl -e 'alarm N; exec @ARGV'`).

```sh
cargo run --release
cargo test --release            # local, inclui o teste que usa GPU real
cargo test --release -- --skip gpu   # o que o CI roda
scripts/package-macos.sh        # app universal + dmg em dist/
```

O script de pacote apaga o DMG da mesma versão. Instalação local: `ditto` para `/Applications` e `xattr -dr com.apple.quarantine`.

Variáveis de depuração:

| Variável | Efeito |
|---|---|
| `NERDMINER_CONFIG=<arquivo>` | Usa outra config. Prefira isso para testes, sem tocar na config real |
| `NERDMINER_RATE_LOG=1` | Imprime o hashrate a cada 5 s |
| `NERDMINER_SHOTS=<dir>` | Salva cada tela em RGBA e sai |
| `NERDMINER_LAB=1` | Abre o laboratório ao iniciar |
| `NERDMINER_LAB_SHOTS=<dir>` | Salva uma imagem por estação e sai |
| `NERDMINER_LAB_BENCH=1` / `live` / `cap` | Mede o custo do laboratório |
| `NERDMINER_LAB_CYCLE=1` / `once` | Abre e fecha o laboratório a cada 5 s, ou uma vez só, para testar a liberação de memória |
| `NERDMINER_LAB_LOOK=dark` | Tema escuro do estúdio |
| `NERDMINER_TRAY_DUMP=1` | Imprime o menu da bandeja no stderr |

Memória no macOS: meça com `footprint -p <pid>`, não com `ps`. O RSS varia muito por causa da compressão de memória. `powermetrics` exige sudo, então peça para o usuário rodar.

## Mapa do código

`main.rs` carrega a config, cria `state::Shared` e sobe as threads. Tudo se comunica por `Shared`.

| Arquivo | Papel |
|---|---|
| `src/state.rs` | Estado compartilhado entre UI, Stratum e mineradores |
| `src/stratum.rs` | Cliente Stratum v1 mínimo, trata `clean_jobs` |
| `src/work.rs` | Cabeçalho, midstate, `Hasher`, `verify()`, dificuldade |
| `src/miner.rs` | Threads de CPU com limite de uso por ciclo de trabalho, cálculo do hashrate |
| `src/gpu.rs`, `src/sha256d.wgsl` | Minerador wgpu (Metal, Vulkan, DX12). O shader devolve candidatos e a CPU reconfere |
| `src/netinfo.rs` | Preço, blocos, mempool e taxas via mempool.space |
| `src/app.rs`, `src/screens.rs` | Janela e telas do dispositivo (egui) |
| `src/tray.rs` | Bandeja: barra de menus no macOS, área de notificação no Windows, SNI no Linux |
| `src/i18n.rs`, `src/fmt.rs`, `src/currency.rs`, `src/config.rs` | Textos, formatação, moeda, config |
| `src/lab/` | Laboratório de Mineração, janela 3D sob demanda |

Laboratório: `mod.rs` (janela e controles), `stations.rs` (as 8 estações), `live.rs` (amostra hashes reais), `model.rs` e `mesh.rs` (peças procedurais), `studio.rs` e `studio.wgsl` (renderizador wgpu próprio), `scene.rs` (câmera e projeção dos rótulos), `text.rs` (lições).

## Publicar

1. Push em `main` roda o CI nos 3 sistemas (`build.yml`). O site sai de `site/` por `pages.yml`.
2. Para versão nova: suba `version` no `Cargo.toml`, atualize docs e site, e crie a tag `vX.Y.Z`. `release.yml` gera dmg, tar.gz e zip.
3. Depois, `gh release edit` com notas em en e pt, incluindo limites conhecidos.

Seções do site, pela numeração `.idx`: 01 telas, 02 loteria, 03 recursos, 04 status, 05 laboratório, 06 download, 07 instalação, 08 FAQ.

Prévia do site: `python3 -I -m http.server` em `site/` e Chrome headless. Mobile precisa de navegador de verdade, porque o headless limita a largura mínima. O app no macOS tem assinatura ad-hoc, sem notarização.
