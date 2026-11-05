# Roadmap

Ideias aprovadas ou propostas, sem data. Marque aqui o que entrar em uma versão.

## Próximas

- **Consumo de energia (estimativa em watts).** Mostrar no app quantos watts a mineração consome, estimados a partir do uso configurado de CPU/GPU e do modelo detectado. É uma estimativa: deixar isso claro na interface. Decisão: partir do TDP anunciado (TDP × threads ÷ total × % de uso; GPU: TDP × %), sem precisão de medição. No macOS não há leitura de potência sem privilégios, e a Apple não publica TDP, então os chips Apple precisam de outro critério.
- **Hashrate cortado nas telas.** A partir de 100 KH/s o primeiro dígito é cortado na caixa amarela.

## Depois

- **Barra de menus, fase 2:** painel popover com gráficos.
- **Barra de menus, fase 3:** notificações e recordes históricos.
- **Mais idiomas:** por exemplo, espanhol, em `src/i18n.rs` e `src/lab/text.rs`.
- **Manual em inglês:** traduzir os exemplos que ainda estão em português.
