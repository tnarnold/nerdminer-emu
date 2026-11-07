# Roadmap

Ideias aprovadas ou propostas, sem data. Marque aqui o que entrar em uma versão. Mapa do projeto e regras de trabalho: [`CLAUDE.md`](../CLAUDE.md).

## Próximas

Em ordem sugerida. As três primeiras são pequenas.

- **Hashrate cortado nas telas.** A partir de 100 KH/s o primeiro dígito é cortado na caixa amarela.
- **Memória nos READMEs.** `README.md` e `README.pt-BR.md` ainda dizem ~40 MB e ~28–45 MB. O site já foi corrigido: `footprint` mediu ~50 MB na v0.4.0 sem minerar. Medir de novo por configuração antes de trocar a tabela.
- **Mais hashrate na GPU (saída antecipada e pré-cálculo).** Detalhes em [Eficiência de mineração](#eficiência-de-mineração).
- **Consumo de energia (estimativa em watts).** Mostrar no app quantos watts a mineração consome, estimados a partir do uso configurado de CPU/GPU e do modelo detectado. É uma estimativa: deixar isso claro na interface. Decisão: partir do TDP anunciado (TDP × threads ÷ total × % de uso; GPU: TDP × %), sem precisão de medição. No macOS não há leitura de potência sem privilégios, e a Apple não publica TDP, então os chips Apple precisam de outro critério.

## Laboratório: pendências da v0.4.0

- **Rodar no Linux e no Windows.** O laboratório só foi executado no macOS; o CI apenas compila e testa.
- **Plano B para GPU fraca.** Hoje não há alternativa se o renderizador do estúdio não rodar bem.
- **Memória depois de fechar.** Sobram ~29 MB após fechar a janela. Investigar se dá para devolver mais.
- **Teste manual completo.** Mouse, zoom, fechar pelo X e abrir pela bandeja ainda não foram confirmados à mão.

## Eficiência de mineração

Não existe atalho conhecido para o SHA-256 duplo: cada hash tem a mesma chance (alvo ÷ 2²⁵⁶), e a ordem dos nonces não importa. Só ajuda fazer mais hashes por segundo ou desperdiçar menos. Os ganhos abaixo são estimativas, nada foi medido, e as referências foram citadas de memória: confira antes de publicar no site ou no laboratório.

### Já em uso

- Midstate na CPU (`src/work.rs`, `Hasher`) e na GPU (`src/gpu.rs`, `src/sha256d.wgsl`).
- Instruções SHA do processador (ARMv8, SHA-NI), via `compress256`.
- Descarte do trabalho quando a pool manda `clean_jobs` (`src/stratum.rs`).

### A fazer: otimizar o shader

`src/sha256d.wgsl` chama `compress()` duas vezes, com 128 rodadas e 96 palavras de mensagem expandidas por hash. Ganho esperado: uns 5 a 10% na GPU.

1. **Saída antecipada.** Tanto a CPU quanto a GPU só olham a palavra 7 do resultado, e o que passa é reconferido com `verify()`. Na segunda compressão, a palavra 7 só depende do `e` da rodada 60, então as rodadas 61 a 63 e `w[61..63]` podem sair. O melhor hash continua correto, porque o filtro não muda.
2. **Pré-cálculo do bloco com o nonce.** No primeiro bloco, `w[0..2]` são fixos e o nonce é `w[3]`. As rodadas 0 a 2 e as palavras `w[16]` e `w[17]` não dependem do nonce, então podem ser calculadas uma vez por trabalho em `GpuMiner::run` e enviadas em `Params`.
3. **Constantes do segundo hash.** Em `blk3`, as palavras 8 a 15 são fixas, e parte da expansão pode virar constante.

Como validar:

- Medir antes de mudar: o compilador do shader pode já fazer parte disso. Comparar `NERDMINER_RATE_LOG=1` antes e depois, com a mesma configuração, a mineração estável e várias leituras.
- `gpu_finds_genesis_nonce` tem que continuar passando. Esse teste precisa de GPU real e é pulado no CI (`--skip gpu`), então rode localmente.
- Conferir que um candidato achado pelo shader passa em `verify()`.

Na CPU o ganho é bem menor: as instruções SHA fazem as rodadas no hardware, sem como parar antes. Só vale tentar depois da GPU, e medindo.

Depois disso, é um bom tema para o laboratório: mostrar que esses truques aceleram o sorteio, mas não mudam a chance de cada hash.

### Talvez

- **AsicBoost** (Hanke, 2016). Até ~20% segundo o artigo, em ASIC. Precisa de version rolling (BIP 310/320) e de suporte da pool, o que exige `mining.configure` em `src/stratum.rs`, que não existe hoje. O ganho em CPU/GPU é incerto.
- **Pool com latência menor.** Cada 100 ms a menos economizam cerca de 0,02% do tempo. Ínfimo, mas de graça.
- **Mostrar a taxa da pool e as taxas do mempool.** Minerar com mempool cheio aumenta o prêmio, não a chance. Taxa de pool sai direto do prêmio.

### Descartado

Não proponha de novo sem fato novo.

- **SAT solvers** (Heusser, 2013): não ganham da força bruta na prática.
- **Grover e computação quântica** (Aggarwal et al., 2017): exigem máquinas que não existem.
- **Selfish mining** (Eyal e Sirer, 2013): só compensa com fatia grande da rede.
- **Ordem ou faixas de nonce "da sorte":** não há padrão.
- **Pool compartilhada para "aumentar a chance":** reduz a variância, não o valor esperado. Além disso, o produto é loteria solo.

Referência principal: *Optimizing SHA256 in Bitcoin Mining* (Courtois, Grajek e Naik, 2014).

## Depois

- **Barra de menus, fase 2:** painel popover com gráficos.
- **Barra de menus, fase 3:** notificações e recordes históricos.
- **Mais idiomas:** por exemplo, espanhol, em `src/i18n.rs` e `src/lab/text.rs`.
- **Manual em inglês:** traduzir os exemplos que ainda estão em português.
