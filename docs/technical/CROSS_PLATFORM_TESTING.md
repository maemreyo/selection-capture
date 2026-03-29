# 🧪 Cross-Platform Testing Strategy

## Testing Windows/Linux Without Physical Machines

---

## 🎯 Problem Statement

**Challenge:** You want to add Windows and Linux support to `selection-capture`, but you only have a Mac.

**Good news:** This is VERY common in the Rust community! Many successful cross-platform crates are developed this way.

---

## ✅ Solutions (Ranked by Effectiveness)

### **Solution 1: GitHub Actions CI/CD** ⭐⭐⭐⭐⭐

**Best for:** Automated testing on all platforms

**How it works:**
```yaml
# .github/workflows/ci.yml
jobs:
  test-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test

  test-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
```

**Pros:**
- ✅ FREE for public repos
- ✅ Real Windows/Linux environments
- ✅ Runs on every push/PR
- ✅ No setup cost
- ✅ Automatic validation

**Cons:**
- ⏱️ Time-limited (6 hours max job)
- 📊 Limited debugging (CI logs only)
- 🖥️ No GUI access

**Cost:** FREE (2000 minutes/month for free accounts)

---

### **Solution 2: Local VMs (Virtual Machines)** ⭐⭐⭐⭐

**Best for:** Interactive debugging and manual testing

#### **Option A: VirtualBox (FREE)**
```bash
# Install VirtualBox
brew install virtualbox  # macOS

# Download Windows 11 Dev VM (free for 90 days)
# https://developer.microsoft.com/en-us/windows/downloads/virtual-machines/

# Download Ubuntu Desktop
# https://ubuntu.com/download/desktop
```

**Setup:**
1. Install VirtualBox
2. Download VM images
3. Install guest OS
4. Install Rust toolchain inside VM
5. Test your code!

**Pros:**
- ✅ FREE
- ✅ Full GUI access
- ✅ Snapshot support (save state)
- ✅ Unlimited testing time

**Cons:**
- 💾 Heavy (20-40GB per VM)
- 🐌 Slower than native
- 🔧 Manual setup required

---

#### **Option B: UTM (macOS ARM)** ⭐⭐⭐⭐⭐
```bash
# For Apple Silicon Macs
brew install utm

# Download Windows 11 ARM
# https://www.microsoft.com/software-download/windows11ARM
```

**Why UTM is great for M1/M2/M3:**
- Native ARM performance
- Windows 11 ARM runs natively
- Much faster than VirtualBox on ARM

**Cost:** FREE

---

#### **Option C: Parallels Desktop** (Paid, but worth it)
```bash
# Parallels Desktop for Mac
# https://www.parallels.com/
```

**Pros:**
- ⚡ Best performance
- 🎯 Seamless integration
- 🖥️ Coherence mode (Windows apps alongside macOS)
- 🆓 Free updates

**Cost:** ~$100/year (or $15 one-time for old version)

**Student discount:** FREE if you're a student!

---

### **Solution 3: Docker for Linux** ⭐⭐⭐⭐

**Best for:** Linux testing (server/desktop)

```bash
# Run Linux tests in Docker
docker run --rm -it rust:latest bash

# Inside container:
cargo new test-project
cd test-project
cargo add selection-capture
cargo test
```

**Advanced: Docker Compose for multi-distro testing**
```yaml
# docker-compose.yml
version: '3'
services:
  ubuntu-22:
    image: rust:1.75-bullseye
    volumes:
      - .:/src
    command: cargo test

  ubuntu-20:
    image: rust:1.75-buster
    volumes:
      - .:/src
    command: cargo test

  alpine:
    image: rust:alpine
    volumes:
      - .:/src
    command: cargo test
```

**Run all at once:**
```bash
docker-compose up --abort-on-container-exit
```

**Pros:**
- ✅ Lightweight (no full OS)
- ✅ Fast startup
- ✅ Easy to switch distros
- ✅ Scriptable

**Cons:**
- ❌ No GUI (terminal only)
- ❌ Can't test desktop-specific features easily

**Cost:** FREE

---

### **Solution 4: Cloud VMs** ⭐⭐⭐

**Best for:** Long-running tests, heavy workloads

#### **GitHub Codespaces** (FREE tier)
```bash
# Open your repo in Codespaces
# https://github.com/{user}/selection-capture/codespaces
```

**Free tier:**
- 60 hours/month FREE
- 2-core machine
- Full VS Code in browser

**Pros:**
- ✅ Pre-configured dev environment
- ✅ VS Code web interface
- ✅ Port forwarding
- ✅ Persistent storage

**Cons:**
- ⏱️ Limited hours
- ❌ Windows not available (Linux only)

---

#### **AWS EC2 Free Tier** (12 months FREE)
```bash
# Launch t2.micro or t3.micro instance
# Windows Server or Amazon Linux
# ssh into it and test
```

**Free tier limits:**
- 750 hours/month (24/7 for 1 month)
- t2.micro or t3.micro
- Windows or Linux

**Pros:**
- ✅ Real cloud environment
- ✅ Choose any OS
- ✅ 24/7 access

**Cons:**
- 💰 Credit card required
- ⏰ Only 12 months free
- 🔧 More setup required

---

#### **Azure Free Account** ($200 credit)
```bash
# Sign up at https://azure.microsoft.com/free/
# Get $200 credit for first 30 days
# Deploy Windows 10/11 VM
```

**Pros:**
- ✅ $200 FREE credit
- ✅ Windows VMs available
- ✅ 12 months free services

**Cons:**
- 💳 Credit card required
- ⏰ Credit expires in 30 days

---

### **Solution 5: Remote Desktop to Friends' Machines** ⭐⭐

**Best for:** Occasional testing

**Ask around:**
- Friends with Windows PCs
- Colleagues with Linux laptops
- Local meetups
- Discord communities

**Tools:**
- SSH (Linux/Mac)
- Remote Desktop (Windows)
- Tailscale (secure VPN)
- ngrok (tunneling)

**Example with Tailscale:**
```bash
# Install Tailscale on both machines
# tailscale.com

# Your friend's Windows machine:
tailscale up

# Your Mac:
ssh friend@windows-pc
cargo test
```

**Cost:** FREE

**Pros:**
- ✅ Real hardware
- ✅ No setup cost
- 🤝 Community building

**Cons:**
- 👥 Depends on others' availability
- 🔒 Security considerations

---

### **Solution 6: Hire Remote Testers** ⭐⭐

**Best for:** Final validation before release

**Platforms:**
- **Fiverr** ($5-20/test session)
- **Upwork** ($10-50/hour)
- **r/forhire** (Reddit)
- **Discord servers**

**Typical job post:**
```
Looking for Windows/Linux user to test Rust crate

Budget: $20 for 1 hour
Task: Run my test suite and report results
Requirements: Windows 10/11 or Ubuntu 22.04
```

**Cost:** $20-100 per release

**Pros:**
- ✅ Real users on real hardware
- ✅ Catch platform-specific bugs
- ✅ No maintenance cost

**Cons:**
- 💰 Costs money
- 👥 Coordination overhead

---

## 🎯 Recommended Strategy (What I'd Do)

### **Phase 1: Development (Now - First Windows Release)**

**Daily workflow:**
1. **Develop on Mac** (your main machine)
2. **Use cross-compilation** for syntax checks
   ```bash
   # Add Windows target
   rustup target add x86_64-pc-windows-msvc
   
   # Check compilation (doesn't run, just checks)
   cargo check --target x86_64-pc-windows-msvc
   ```
3. **Push to GitHub** frequently
4. **Let GitHub Actions test** on Windows/Linux
   ```bash
   # CI automatically runs:
   # - Windows tests (windows-latest)
   # - Linux tests (ubuntu-latest)
   # - macOS tests (macos-latest)
   ```

**Time commitment:** 0 extra cost, 5 min setup

---

### **Phase 2: Debugging (When Tests Fail)**

**When CI shows Windows failures:**

**Option A: GitHub Actions Debugging**
```yaml
# Add debug output to CI
- name: Debug Windows failure
  run: |
    echo "Current directory:"
    pwd
    echo "Rust version:"
    rustc --version
    echo "Environment variables:"
    env
  shell: bash
```

**Option B: Use Windows Sandbox** (Windows Pro only)
```bash
# Enable Windows Sandbox feature
# Run clean Windows environment
# Test your code
# Close sandbox (auto-deletes)
```

**Option C: Set up local VM**
- Install VirtualBox/UTM
- Load Windows ISO
- Debug interactively

---

### **Phase 3: Pre-Release Validation**

**Before releasing v0.2.0 (Windows beta):**

1. **Hire tester on Fiverr** ($20)
   - Give them test script
   - They record screen + share results
   - Fix any issues they find

2. **Ask community for beta testers**
   - Post on r/rust
   - Discord channels
   - Twitter/X

3. **Use your network**
   - Ask friends/colleagues
   - Offer pizza/coffee in exchange

---

## 🛠️ Practical Implementation Guide

### **Step 1: Update CI/CD** (DO THIS NOW)

Edit `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  # Current macOS tests
  test-macos:
    name: Test (macOS)
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --verbose
      - run: cargo test --verbose

  # ADD: Windows tests
  test-windows:
    name: Test (Windows)
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --verbose
      - run: cargo test --verbose

  # ADD: Linux tests
  test-linux:
    name: Test (Linux)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --verbose
      - run: cargo test --verbose

  # ADD: Cross-compilation check
  check-cross-compile:
    name: Cross-compile check
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Add Windows target
        run: rustup target add x86_64-pc-windows-msvc
      - name: Check Windows compilation
        run: cargo check --target x86_64-pc-windows-msvc
      - name: Add Linux target
        run: rustup target add x86_64-unknown-linux-gnu
      - name: Check Linux compilation
        run: cargo check --target x86_64-unknown-linux-gnu
```

**This gives you:**
- ✅ Automatic testing on all 3 platforms
- ✅ Cross-compilation syntax checks
- ✅ Platform-specific bug detection
- ✅ All FREE via GitHub Actions

---

### **Step 2: Set Up Local VM** (Optional but Recommended)

**For Apple Silicon (M1/M2/M3):**

```bash
# Install UTM
brew install utm

# Download Windows 11 ARM
# https://www.microsoft.com/software-download/windows11ARM

# Create VM in UTM:
# 1. New → Virtualize
# 2. Select Windows 11 ARM ISO
# 3. Allocate 4GB RAM, 64GB disk
# 4. Install Windows
# 5. Install Rust: winget install Rustlang.Rust.MSVC
```

**For Intel Macs:**

```bash
# Install VirtualBox
brew install virtualbox

# Download Windows 10/11 VM
# https://developer.microsoft.com/en-us/windows/downloads/virtual-machines/

# Import VM and start testing
```

**Time investment:** 1-2 hours setup  
**Benefit:** Unlimited local testing

---

### **Step 3: Use Docker for Quick Linux Checks**

```bash
# Create Makefile target
test-linux-docker:
	docker run --rm -it \
		-v $(pwd):/src \
		rust:latest \
		bash -c "cd /src && cargo test"

# Usage:
make test-linux-docker
```

**Quick iteration:**
```bash
# Edit code on Mac
vim src/lib.rs

# Test in Linux container
make test-linux-docker

# Repeat until green
```

---

## 💡 Tips for Success

### **Tip 1: Write Platform-Agnostic Code**

```rust
// GOOD: Abstract behind trait
impl CapturePlatform for WindowsPlatform {
    fn attempt(&self, method: CaptureMethod) -> PlatformAttemptResult {
        // Windows-specific implementation
    }
}

// BAD: Hardcoded paths
let path = "/usr/bin/something"; // Won't work on Windows!
```

---

### **Tip 2: Use Conditional Compilation**

```rust
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;
```

---

### **Tip 3: Test Logic Separately from Platform Code**

```rust
// GOOD: Pure functions (easy to test)
fn parse_selected_text(raw: &str) -> Option<String> {
    // Platform-agnostic logic
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_selected_text() {
        // Works on any platform!
        assert_eq!(parse_selected_text("hello"), Some("hello".to_string()));
    }
}
```

---

### **Tip 4: Mock Platform Dependencies**

```rust
// In tests:
struct MockWindowsApi;

impl WindowsApi for MockWindowsApi {
    fn get_selected_text(&self) -> Result<String> {
        Ok("mocked text".to_string())
    }
}

#[test]
fn test_windows_capture() {
    let mock = MockWindowsApi;
    let result = capture_with_api(&mock);
    assert_eq!(result, Ok("mocked text".to_string()));
}
```

---

### **Tip 5: Leverage Community**

**Post in your README:**
```markdown
## 🧪 Testing Help Wanted!

I'm developing `selection-capture` primarily on macOS. Looking for volunteers to help test on:
- Windows 10/11
- Ubuntu 22.04+
- Other Linux distros

If you can spare 30 minutes to run tests and report issues, please open an issue!

🎁 Contributors will be acknowledged in the README!
```

---

## 📊 Cost Breakdown

| Solution | Cost | Time | Effectiveness |
|----------|------|------|---------------|
| **GitHub Actions** | FREE | 5 min setup | ⭐⭐⭐⭐⭐ |
| **Docker (Linux)** | FREE | 10 min setup | ⭐⭐⭐⭐ |
| **UTM (ARM VM)** | FREE | 1-2 hours | ⭐⭐⭐⭐ |
| **VirtualBox** | FREE | 1-2 hours | ⭐⭐⭐ |
| **Parallels** | $100/year | 30 min | ⭐⭐⭐⭐⭐ |
| **AWS Free Tier** | FREE (12mo) | 30 min | ⭐⭐⭐ |
| **Codespaces** | FREE (60h/mo) | 5 min | ⭐⭐⭐ |
| **Fiverr Tester** | $20/release | Per release | ⭐⭐⭐ |

---

## 🎯 My Recommendation for YOU

### **Start with this combo:**

1. **GitHub Actions** (immediate, FREE)
   - Automatic testing on every push
   - Catches 90% of issues

2. **UTM/VirtualBox** (weekend project, FREE)
   - For interactive debugging
   - Test edge cases manually

3. **Fiverr tester** (before major releases, $20)
   - Final validation
   - Real user perspective

**Total cost:** $20 per major release  
**Time investment:** 2-3 hours initial setup

---

## 🚀 Action Plan

### **Week 1: Setup**
- [ ] Update CI/CD with Windows/Linux jobs
- [ ] Verify cross-compilation works
- [ ] Push code, watch CI run on all platforms

### **Week 2: Development**
- [ ] Implement Windows platform code
- [ ] Implement Linux platform code
- [ ] Rely on CI for testing
- [ ] Use Docker for quick Linux checks

### **Week 3: Debugging**
- [ ] Fix Windows failures from CI logs
- [ ] Fix Linux failures from CI logs
- [ ] Set up UTM/VirtualBox if needed

### **Week 4: Pre-release**
- [ ] Hire Fiverr tester for final validation
- [ ] Fix any remaining issues
- [ ] Release v0.2.0 with confidence!

---

## 💬 Encouragement

**You're not alone!** Many successful cross-platform Rust crates were built this way:

- **ripgrep** - Andrew Kelley developed primarily on Linux, tested via CI
- **bat** - Developed on Mac, Windows/Linux support via GitHub Actions
- **fd** - Same pattern
- **tealdeer** - Built on Mac, tested on all platforms via CI

**The Rust community has excellent tooling for this exact scenario!**

---

*Created: 2026-03-29*  
*Author: zamery (zaob.ogn@gmail.com)*  
*License: MIT OR Apache-2.0*

---

## Appendix: Quick Start Commands

```bash
# 1. Add Windows cross-compilation target
rustup target add x86_64-pc-windows-msvc

# 2. Check if your code compiles for Windows
cargo check --target x86_64-pc-windows-msvc

# 3. Add Linux target
rustup target add x86_64-unknown-linux-gnu

# 4. Check Linux compilation
cargo check --target x86_64-unknown-linux-gnu

# 5. Test in Docker (Linux)
docker run --rm -it -v $(pwd):/src rust:latest bash -c "cd /src && cargo test"

# 6. Push to GitHub and let CI do the rest!
git push
```

**That's it! You can develop cross-platform without owning multiple machines!** 🎉
