package com.viaembedded.smartetk;

/* loaded from: classes.dex */
final class PacketList {
    private final Node m_firstNode;
    private Node m_lastNode;

    PacketList() {
        Node node = new Node();
        this.m_firstNode = node;
        this.m_lastNode = node;
    }

    static class Node {
        Node m_next = null;

        Node() {
        }
    }

    synchronized boolean push(Packet packet) {
        if (packet == null) {
            return false;
        }
        this.m_lastNode.m_next = packet;
        this.m_lastNode = packet;
        packet.m_next = null;
        return true;
    }

    synchronized boolean remove(Packet packet) {
        if (packet != null) {
            for (Node node = this.m_firstNode; node != null && node.m_next != null; node = node.m_next) {
                if (node.m_next == packet) {
                    removeNext(node);
                    return true;
                }
            }
        }
        return false;
    }

    private synchronized Packet removeFirst() {
        return removeNext(this.m_firstNode);
    }

    private synchronized Packet getPacket(byte[] bArr, int i) {
        for (Node node = this.m_firstNode; node != null && node.m_next != null; node = node.m_next) {
            if (((Packet) node.m_next).checkPacket(bArr, i)) {
                return removeNext(node);
            }
        }
        return null;
    }

    private synchronized Packet removeNext(Node node) {
        Node node2;
        if (node != null) {
            try {
                node2 = node.m_next;
            } catch (Throwable th) {
                throw th;
            }
        } else {
            node2 = null;
        }
        if (node != null && node2 != null) {
            node.m_next = node2.m_next;
            if (node2.m_next == null) {
                this.m_lastNode = node;
            } else {
                node2.m_next = null;
            }
        }
        return (Packet) node2;
    }

    boolean notifyError(byte[] bArr, int i, int i2) {
        Packet packet = getPacket(bArr, i);
        if (packet != null) {
            return packet.notifyError(bArr, i, i2);
        }
        return false;
    }

    boolean notifyPacket(byte[] bArr, int i) {
        Packet packet = getPacket(bArr, i);
        if (packet != null) {
            return packet.notifyPacket(bArr, i);
        }
        return false;
    }

    void notifyAll(int i) {
        while (true) {
            Packet removeFirst = removeFirst();
            if (removeFirst == null) {
                return;
            } else {
                removeFirst.notifyError(i);
            }
        }
    }
}
