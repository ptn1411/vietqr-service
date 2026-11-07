# VietQR Service & Web Studio (Rust Native)

<p align="center">
  <a href="https://github.com/ptn1411/vietqr-service/releases"><img src="https://img.shields.io/github/v/release/ptn1411/vietqr-service?color=047857" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-0284c7.svg" alt="License: MIT"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-ea580c.svg" alt="Rust: 1.75+"></a>
</p>

Dịch vụ độc lập hiệu năng cao viết bằng **Rust**, chuyên tạo mã QR thanh toán chuẩn **VietQR (NAPAS247)** và tạo liên kết mở trực tiếp **33+ ứng dụng ngân hàng Việt Nam (Deeplink)**.

Hỗ trợ **tùy biến 2 dòng chữ chân trang**, **chèn ảnh logo tùy chỉnh ở giữa mã QR**, các mẫu **SePay / PTN Standee Full Color**, xuất file PNG siêu nhẹ (~4-6 KB) và tích hợp sẵn **Web Studio** trực quan nhúng trực tiếp trong file binary duy nhất.

---

## 📦 Tải Về & Khởi Chạy Ngay (Pre-built Binaries)

Không cần cài đặt Rust hay Node.js! Tải file nhị phân biên dịch sẵn từ trang [GitHub Releases](https://github.com/ptn1411/vietqr-service/releases):

| Hệ điều hành | Kiến trúc | Tên file | Mô tả |
| :--- | :--- | :--- | :--- |
| **Windows** | x86_64 / amd64 | `vietqr-service-*-windows-amd64.zip` | Chạy trực tiếp trên Windows 10/11, Windows Server |
| **Linux (glibc)** | x86_64 / amd64 | `vietqr-service-*-linux-amd64.tar.gz` | Ubuntu, Debian, CentOS, AlmaLinux, Fedora |
| **Linux (Static)** | x86_64 / amd64 | `vietqr-service-*-linux-musl-amd64.tar.gz` | Độc lập 100%, chạy trên Alpine Linux & Docker scratch |

### 1. Trên Windows
1. Tải file `.zip` từ mục [Releases](https://github.com/ptn1411/vietqr-service/releases).
2. Giải nén và nhấp đúp file `vietqr-service.exe` (hoặc chạy trong PowerShell: `.\vietqr-service.exe`).
3. Mở trình duyệt truy cập: `http://localhost:8090`.

### 2. Trên Linux VPS / Server
```bash
# Tải và giải nén (thay thế URL phiên bản mới nhất)
wget https://github.com/ptn1411/vietqr-service/releases/latest/download/vietqr-service-linux-amd64.tar.gz
tar -xzf vietqr-service-linux-amd64.tar.gz
cd vietqr-service-*

# Phân quyền và khởi chạy
chmod +x vietqr-service
./vietqr-service
```

> **Đổi cổng dịch vụ**: Thiết lập biến môi trường `PORT`:
> ```bash
> PORT=8088 ./vietqr-service
> ```

### 3. Cài Đặt Làm Dịch Vụ Nền (systemd Service trên Linux)
Tạo file cấu hình dịch vụ `/etc/systemd/system/vietqr.service`:
```ini
[Unit]
Description=VietQR High-Performance Service
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/vietqr-service
ExecStart=/opt/vietqr-service/vietqr-service
Restart=always
RestartSec=3
Environment=PORT=8090

[Install]
WantedBy=multi-user.target
```
Kích hoạt và khởi động:
```bash
systemctl daemon-reload
systemctl enable --now vietqr
```

---

## 🌟 Tính Năng Nổi Bật

1. **Chuẩn VietQR EMVCo / NAPAS247**:
   - Hỗ trợ đầy đủ mã định danh 65+ ngân hàng và tổ chức tài chính Việt Nam (Vietcombank, TPBank, MB, Techcombank, BIDV, Agribank, ACB, VPBank, Timo, CAKE...).
   - Khử dấu tiếng Việt, chuẩn hóa thông tin người thụ hưởng, giới hạn độ dài an toàn theo chuẩn EMVCo Tag 59 và Tag 62.
   - Tính toán mã kiểm tra CRC16-CCITT (False/0xFFFF) tức thì.

2. **Mẫu Standee & SePay Chuẩn Đồ Họa Cao Cấp**:
   - Tích hợp endpoint `/img` tương thích 100% với tham số chuẩn của SePay: `bank`, `acc`, `template`, `amount`, `des`, `name`.
   - Hỗ trợ các mẫu template phong phú:
     - `template=ptn`: Mẫu thương hiệu SePay Full Color với logo PTN emblem cao cấp, dải logo Napas 24/7, TPBank & VietQR sắc nét.
     - `template=sepay`: Mẫu chuẩn phong cách cổng thanh toán SePay.
     - `template=standee`: Mẫu standee để bàn kèm thông tin ngân hàng thụ hưởng.
     - `template=qronly`: Mã QR tinh gọn 1:1 không kèm viền chân trang.
     - `template=compact`: Tinh gọn cho hóa đơn và màn hình nhỏ.

3. **Chèn Logo Tùy Chỉnh Ở Tâm (Center Logo)**:
   - **Huy hiệu Ngân hàng Tự động**: Tự nhận diện ngân hàng thụ hưởng và vẽ huy hiệu mang màu sắc nhận diện thương hiệu chính xác.
   - **Chữ tùy biến**: Huy hiệu mang nội dung ngắn (VD: `SEPAY`, `VIP`, `NAM`, `PAY`).
   - **Tải ảnh logo lên**: Hỗ trợ upload ảnh logo tùy biến (PNG, JPG, WebP), tự động co dãn tỉ lệ và đệm viền bo tròn trắng an toàn trong ngân sách sửa lỗi Level H (30%) mà camera các app ngân hàng quét mượt 100%.

4. **Bộ Mã Hóa PNG Siêu Nhẹ (~4-6 KB)**:
   - Sử dụng bảng màu Indexed-Color Palette (`color_type = 3`, 16 màu, `PLTE` chunk 48 byte).
   - Bộ lọc quét dòng PNG Up Filter (`filter_type = 2`) triệt tiêu dung lượng các khối module liên tiếp.
   - Nén Zlib Level 9 (`Compression::best()`), nhẹ hơn 80% so với ảnh xuất từ canvas thông thường.
   - Chế độ Truecolor RGB tối ưu khi chèn ảnh logo nhiều màu (~8-12 KB).

5. **Deeplink Engine 33+ App Ngân Hàng**:
   - Hỗ trợ Android `intent://` và iOS URL Scheme cho 33+ app ngân hàng phổ biến nhất Việt Nam.
   - Tự động điều hướng và fallback thông minh về trang thanh toán web tương tác.

6. **Web Studio Hiện Đại (Giao diện Sáng Sang Trọng)**:
   - Giao diện Web Studio nhúng trực tiếp vào binary, không cần cài đặt Node.js hay phụ thuộc static files bên ngoài.
   - Thiết kế chuẩn Light Theme SaaS (WCAG AA compliant) tối giản, hiện đại.
   - Xem trước realtime (debounce 200ms), tải ảnh PNG, sao chép payload EMVCo và copy link thanh toán nhanh chóng.

---

## 🛠️ Biên Dịch Từ Mã Nguồn (Developer)

### Yêu Cầu
- [Rust](https://www.rust-lang.org/) phiên bản 1.75 trở lên.

### Khởi Chạy
```bash
git clone https://github.com/ptn1411/vietqr-service.git
cd vietqr-service
cargo run --release
```

Server sẽ tự động khởi động tại cổng `8090` (hoặc cổng cấu hình qua biến môi trường `PORT`):
```
🚀 VietQR Service & Web Studio listening on http://0.0.0.0:8090
```

Mở trình duyệt truy cập: `http://localhost:8090` để vào giao diện **VietQR Studio**.

---

## 📡 RESTful API Documentation

### 1. `GET /img` (SePay Compatible Endpoint)
Tạo ảnh QR tương thích hoàn toàn với chuẩn gọi ảnh của SePay.

**Query Parameters:**
| Tham số | Kiểu | Mô tả |
| :--- | :--- | :--- |
| `bank` | string | Mã ngân hàng thụ hưởng (VD: `TPB`, `VCB`, `MB`, `TCB`...) |
| `acc` | string | Số tài khoản thụ hưởng |
| `template` | string | Mẫu giao diện: `ptn`, `sepay`, `standee`, `qronly`, `compact` |
| `amount` | integer | Số tiền (VNĐ). Nếu bỏ trống hoặc = 0, khách tự nhập khi quét |
| `des` | string | Nội dung chuyển khoản (tự chuẩn hóa an toàn) |
| `name` | string | Tên chủ tài khoản |

**Ví dụ:**
```bash
curl "http://localhost:8090/img?bank=TPB&acc=69238686888&template=ptn&amount=500000&des=DONATE&name=PHAM%20THANH%20NAM" -o qr_ptn.png
```

---

### 2. `GET /api/qr.png`
Tạo và xuất ảnh mã QR dạng PNG với khả năng tùy biến 2 dòng chân trang.

**Query Parameters:**
| Tham số | Kiểu | Mặc định | Mô tả |
| :--- | :--- | :--- | :--- |
| `bank` | string | `TPB` | Mã ngân hàng (VD: `TPB`, `VCB`, `MB`, `TCB`, `BIDV`...) |
| `acc` | string | `69238686888` | Số tài khoản thụ hưởng |
| `amount` | integer | `0` | Số tiền (VNĐ). Nếu = 0, khách tự nhập khi quét |
| `memo` | string | rỗng | Nội dung chuyển khoản (tự khử dấu và ký tự lạ) |
| `name` | string | rỗng | Tên chủ tài khoản (tự chuyển in hoa không dấu) |
| `footer1` | string | rỗng | Dòng chữ chân trang 1 |
| `footer2` | string | rỗng | Dòng chữ chân trang 2 |
| `badge` | string | rỗng | Chữ huy hiệu tâm (nếu muốn ghi đè logo ngân hàng) |
| `scale` | integer | `8` | Tỉ lệ module pixel (từ 4 đến 16) |
| `square` | boolean | `false` | `true` để xuất hình vuông 1:1 không có footer |

---

### 3. `POST /api/qr.png` (Multipart Form Data)
Tạo mã QR hỗ trợ **upload logo tùy chỉnh** (PNG, JPG, WebP) đặt vào tâm QR.

---

### 4. `GET /api/pay`
Trả về dữ liệu JSON gồm chuỗi EMVCo payload, URL ảnh QR và Deeplink mở app tương ứng.

---

### 5. `GET /qr/pay`
Trang thanh toán tương tác dành cho thiết bị di động (Mobile Web Hub):
- Tự động mở app ngân hàng người mua đã chọn nếu mở trên smartphone.
- Hiển thị nút "Lưu ảnh vào Album" để quét QR qua gallery ảnh (chuẩn nhận diện tự điền 100% mọi ngân hàng).
- Sao chép nhanh số tài khoản, số tiền và nội dung chuyển khoản chỉ với một chạm.

---

## 🧪 Kiểm Thử (Unit & Integration Tests)

Chạy bộ kiểm thử toàn diện:
```bash
cargo test
```
Toàn bộ 31 bài test kiểm tra TLV, CRC16, 65+ BIN, khử dấu tiếng Việt, nén PNG, Deeplink 33+ ngân hàng, số tiền lớn và toàn bộ API endpoint.

---

## 📄 Bản Quyền & Tác Giả

Dự án được xây dựng và phát triển bởi **Phạm Thành Nam**.
Mã nguồn phần mềm được phát hành theo giấy phép **MIT License**.

---

## ⚖️ Tuyên Bố Bản Quyền, Nhãn Hiệu & Miễn Trừ Trách Nhiệm (Legal Disclaimer)

1. **Chuẩn Kỹ Thuật & Quyền Tác Giả Phần Mềm**:
   - Thuật toán tạo chuỗi thanh toán tuân thủ quy chuẩn kỹ thuật tại **Quyết định số 1928/QĐ-NHNN ngày 05/10/2018** của Thống đốc Ngân hàng Nhà nước Việt Nam và đặc tả mở **EMVCo Merchant-Presented Mode (MPM)**.
   - Theo Điều 15 khoản 2 Luật Sở hữu trí tuệ Việt Nam, các văn bản quy chuẩn hành chính, nguyên lý, quy trình kỹ thuật không thuộc đối tượng bảo hộ quyền tác giả. Mã nguồn dịch vụ này được lập trình mới 100% bằng ngôn ngữ Rust (Clean-room implementation), hoàn toàn độc lập và không sao chép từ bất kỳ SDK đóng hay dịch vụ nội bộ nào.

2. **Nhãn Hiệu Thương Mại & Nhận Diện Logo**:
   - **VietQR** và **NAPAS 24/7** là các nhãn hiệu đã đăng ký thuộc quyền sở hữu của **Công ty Cổ phần Thanh toán Quốc gia Việt Nam (NAPAS)**.
   - Logo, tên gọi và bộ nhận diện thương hiệu của các ngân hàng thương mại (TPBank, Vietcombank, Techcombank, MBBank...) thuộc quyền sở hữu của các ngân hàng tương ứng.
   - Việc hiển thị huy hiệu, tên ngân hàng và logo VietQR/NAPAS trên mã QR và các ấn phẩm Standee là nhằm mục đích **chỉ dẫn khả năng tương thích thanh toán (Indicative Use / Fair Use)** theo hướng dẫn nhận diện thương hiệu VietQR của NAPAS, phục vụ người dùng quét mã thanh toán chính xác.
   - Dự án là một tiện ích công nghệ mã nguồn mở độc lập, **KHÔNG PHẢI** là dịch vụ chính thức, không đại diện và không có mối quan hệ liên kết hay ủy quyền tài chính với NAPAS, SePay hay Ngân hàng Nhà nước Việt Nam.

3. **Bảo Vệ Dữ Liệu Cá Nhân (Privacy by Design)**:
   - Dịch vụ tuân thủ nghiêm ngặt **Luật Bảo vệ dữ liệu cá nhân số 91/2025/QH15** và **Nghị định 356/2025/NĐ-CP**: Toàn bộ quá trình tạo chuỗi EMVCo và render ảnh mã QR được xử lý **100% trong bộ nhớ tạm (In-Memory)**. Hệ thống không sử dụng cơ sở dữ liệu, không ghi nhật ký thông tin nhạy cảm ra ổ đĩa (Zero Disk Logging), và không theo dõi hành vi người dùng (Zero Cookies & Tracking Scripts).

---
Nếu dự án giúp ích cho công việc nghiên cứu hoặc phòng thủ của bạn, bạn có thể ủng hộ để duy trì tài liệu, test và các workflow mới. Mọi khoản đóng góp đều hoàn toàn tự nguyện.

- Ngân hàng: **TPBank**
- Chủ tài khoản: **PHAM THANH NAM**
- Số tài khoản: **69238686888**

<p align="center">
  <img src="https://raw.githubusercontent.com/ptn1411/skill/main/qr-sepay.png" alt="QR SePay TPBank" width="300">
</p>
