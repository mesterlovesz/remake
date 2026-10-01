"""The Kiskina capture views (tools/capture_kiskina.py). Positions are native units (1 unit = 1 cm); eyes sit 64 above a navigation-graph floor point."""
from __future__ import annotations


def register(view) -> None:
    # start building and its soldiers
    view("a01-start-retail", None)
    view("a02-start-west", (7520, -1088, -1136), (5500, -1136))
    view("a03-soldier-o_postac1", (7183, -1088, -795), (6829, -780))
    view("a04-soldier-o_postac3", (6781, -1088, -1548), (6990, -1254))
    view("a05-soldier-o_postac2", (7279, -1088, -1323), (6956, -1476))
    view("a06-start-kettle-room", (6511, -1088, -939), (6442, -790))
    view("a07-ring-corridor", (7183, -1088, -843), (6700, -843))
    # the passage and the main street
    view("b01-passage-mason13", (6300, -1086, -1130), (5200, -1130))
    view("b02-street-civilian", (5600, -1088, -1100), (4200, -1200))
    view("b03-street-market", (4300, -1088, -1000), (3400, -1200))
    view("b04-street-back-east", (3700, -1084, -1180), (5800, -1130))
    view("b05-civilian-cywil1", (5503, -1036, -1371), (5493, -1028))
    view("b06-civilian-cywil2", (5119, -1055, -1227), (5116, -916))
    view("b07-civilian-cywil3", (3919, -1084, -1227), (3881, -910))
    view("b08-civilian-cywil4", (3823, -1084, -1083), (3786, -1406))
    view("b09-newspaper-gazeta", (4687, -1079, -1419), (4434, -1163))
    view("b10-qra-o_postac21", (2815, -1084, -1323), (2784, -1008))
    view("b11-qra-o_postac22", (2365, -1084, -1260), (2720, -1328))
    view("b12-tv-room", (2950, -1086, -1520), (2780, -1540))
    # west building, corridor and exit
    view("c01-west-corridor", (2500, -1072, -1230), (700, -1120))
    view("c02-marker-ChinioleOstrzezenie", (2700, -1072, -1236), (2333, -1236))
    view("c03-guard-o_postac10", (1759, -1072, -891), (1373, -914))
    view("c04-guard-o_postac11", (1183, -1072, -891), (1110, -1357))
    view("c05-guard-o_postac9", (1279, -1072, -1611), (1615, -1563))
    view("c06-west-shop", (1700, -1086, -1450), (1880, -1580))
    view("c07-exit-corridor", (1000, -1072, -1100), (200, -1100))
    view("c08-exit-door", (200, -1086, -1000), (128, -640))
    # roofs and the rooks
    view("d01-roof-rook", (4400, -300, -1150), (4128, -1136))
    view("d02-roof-watertower", (4254, -627, -876), (3500, -1300))
    view("d04-roof-east", (4528, -523, -1453), (5600, -1100))
    # the retail dialogue panel (probe scenario `dialogue`: the player is put 110 units in front of the named character definition)
    def talk(name: str, node: str, person: str) -> None:
        view(name, None, None, 9.0, MESTER_TEST_SCENARIO="dialogue", MESTER_TEST_DIALOG=node, MESTER_TEST_PERSON=person, MESTER_TEST_START="1")
    talk("e01-dialog-Chiniole12", "Chiniole12", "china zolnierz2")
    talk("e02-dialog-Chiniole13", "Chiniole13", "china zolnierz")
    talk("e03-dialog-Chiniole8", "Chiniole8", "china zolnierz2")
    talk("e04-dialog-Chiniole20", "Chiniole20", "china zolnierz")
    # A Templom (chinatown2), for the comparison section
    view("f01-templom-start", (-501, -348, -1992), (-501, -1000), world="chinatown2")
    view("f02-templom-gate", (1131, -181, 1128), (1124, 1300), world="chinatown2")
    view("f03-templom-tongpo", (3675, -115, 1018), (3675, 1368), world="chinatown2")
    view("f04-templom-statue", (3008, -284, 864), (3136, 789), world="chinatown2")
