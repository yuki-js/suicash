package com.viaembedded.smartetk;

import com.viaembedded.smartetk.PacketList;

/* loaded from: classes.dex */
final class Packet extends PacketList.Node {
    final byte[] m_byData;
    private byte m_byFun = 0;
    private byte m_byCmd = 0;
    private byte m_bySerialNum = 0;
    private byte[] m_byRecvBuf = null;
    private int m_iRecvOffset = 0;
    int m_iDataLen = 0;
    int m_iStatus = -10;
    private boolean m_bWaitFlag = false;

    Packet(int i) {
        this.m_byData = new byte[i];
    }

    synchronized void setPacket(byte b, byte b2, byte b3, byte[] bArr, int i) {
        this.m_byFun = b;
        this.m_byCmd = b2;
        this.m_bySerialNum = b3;
        this.m_byRecvBuf = bArr;
        this.m_iRecvOffset = i;
        this.m_iDataLen = 0;
        this.m_iStatus = -10;
        this.m_bWaitFlag = true;
    }

    synchronized void disableWaitFlag() {
        this.m_bWaitFlag = false;
    }

    synchronized void waitNotify(int i) {
        try {
            wait(i);
        } catch (InterruptedException e) {
            e.printStackTrace();
        }
        this.m_bWaitFlag = false;
    }

    synchronized void notifyError(int i) {
        if (this.m_bWaitFlag) {
            this.m_iStatus = i;
        }
        notify();
    }

    synchronized boolean notifyError(byte[] bArr, int i, int i2) {
        if (!checkPacket(bArr, i)) {
            return false;
        }
        this.m_iStatus = i2;
        notify();
        return true;
    }

    synchronized boolean notifyPacket(byte[] bArr, int i) {
        if (!setPacket(bArr, i)) {
            return false;
        }
        notify();
        return true;
    }

    boolean checkPacket(byte[] bArr, int i) {
        return this.m_bWaitFlag && bArr != null && bArr[i + 1] == this.m_byFun && bArr[i + 2] == this.m_byCmd && bArr[i + 3] == this.m_bySerialNum;
    }

    private boolean setPacket(byte[] bArr, int i) {
        if (!checkPacket(bArr, i)) {
            return false;
        }
        this.m_iStatus = (bArr[i + 5] & 255) * (-1);
        if (this.m_byRecvBuf == null) {
            return true;
        }
        int i2 = bArr[i + 4] & 255;
        this.m_iDataLen = i2;
        int i3 = i + 7;
        if (-4 == bArr[i + 6]) {
            this.m_iDataLen = SmartETK.byteToInt(bArr, i3, i2);
            i3 += i2;
        }
        int i4 = this.m_iRecvOffset;
        int i5 = this.m_iDataLen;
        int i6 = i4 + i5;
        byte[] bArr2 = this.m_byRecvBuf;
        if (i6 > bArr2.length) {
            this.m_iDataLen = 0;
            this.m_iStatus = -5;
            return true;
        }
        System.arraycopy(bArr, i3, bArr2, i4, i5);
        return true;
    }
}
