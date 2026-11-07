import QtQuick 2.5
import QtQuick.Controls 2.5

Rectangle {
    id: root
    anchors.fill: parent
    color: "white"

    readonly property string termsUrl: "file:///home/root/terms.json"
    property string lastRaw: ""

    signal close
    function unloading() {}

    ListModel { id: cards }

    function show(raw) {
        if (raw === lastRaw)
            return;
        lastRaw = raw;
        cards.clear();
        const terms = JSON.parse(raw).terms || [];
        for (let i = 0; i < terms.length; i++)
            cards.append({ term: terms[i].term, definition: terms[i].definition });
    }

    function poll() {
        const request = new XMLHttpRequest();
        request.onreadystatechange = () => {
            if (request.readyState === XMLHttpRequest.DONE && request.responseText)
                show(request.responseText);
        };
        request.open("GET", termsUrl);
        request.send();
    }

    function clearTerms() {
        const request = new XMLHttpRequest();
        request.open("PUT", termsUrl);
        request.send('{"terms":[]}');
        cards.clear();
    }

    Timer {
        interval: 1500
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: poll()
    }

    Text {
        visible: cards.count === 0
        anchors.centerIn: parent
        width: parent.width - 60
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        font.pointSize: 16
        text: "Highlight words on the page, then tap the lower-right corner."
    }

    ListView {
        anchors.top: parent.top
        anchors.bottom: buttons.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 20
        clip: true
        spacing: 24
        boundsBehavior: Flickable.StopAtBounds
        model: cards
        delegate: Column {
            width: ListView.view.width
            spacing: 4
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                font.pointSize: 18
                font.bold: true
                text: term
            }
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                font.pointSize: 15
                text: definition
            }
        }
    }

    Row {
        id: buttons
        height: 70
        spacing: 16
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.margins: 12

        Rectangle {
            visible: cards.count > 0
            width: 180
            height: parent.height
            border.width: 2
            border.color: "black"

            Text {
                anchors.centerIn: parent
                font.pointSize: 14
                text: "Clear"
            }

            MouseArea {
                anchors.fill: parent
                onClicked: clearTerms()
            }
        }

        Rectangle {
            width: 180
            height: parent.height
            border.width: 2
            border.color: "black"

            Text {
                anchors.centerIn: parent
                font.pointSize: 14
                text: "Close"
            }

            MouseArea {
                anchors.fill: parent
                onClicked: root.close()
            }
        }
    }
}
