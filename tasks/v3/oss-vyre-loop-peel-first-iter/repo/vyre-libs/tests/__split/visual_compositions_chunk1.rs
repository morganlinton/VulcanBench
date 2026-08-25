    // ================================================================
    // Blur
    // ================================================================

    mod blur {
        use vyre_libs::visual::blur::{
            gaussian_blur_2pass, gaussian_blur_2pass_with_kernel, GaussianKernel,
        };

        #[test]
        fn program_has_correct_buffers() {
            let stages = gaussian_blur_2pass("input", "output", "scratch", 8, 8, 2, 1.0);
            assert_eq!(stages.stage_count(), 2);
            let h_bufs = stages.horizontal.buffers();
            assert_eq!(h_bufs.len(), 2, "horizontal pass needs input + scratch");
            assert_eq!(h_bufs[0].name(), "input");
            assert_eq!(h_bufs[1].name(), "scratch");
            let v_bufs = stages.vertical.buffers();
            assert_eq!(v_bufs.len(), 2, "vertical pass needs scratch + output");
            assert_eq!(v_bufs[0].name(), "scratch");
            assert_eq!(v_bufs[1].name(), "output");
            assert_eq!(
                stages.horizontal.workgroup_size(),
                [256, 1, 1],
                "visual kernels must use linear pixel workgroups; dispatch geometry is derived from output buffer size"
            );
            assert_eq!(stages.vertical.workgroup_size(), [256, 1, 1]);
        }

        #[test]
        fn program_radius_zero_has_single_weight() {
            // radius=0 → diameter=1 → only center weight (65536 = 1.0)
            let weights = vyre_primitives::math::conv1d::gaussian_weights(0, 1.0);
            assert_eq!(weights.len(), 1);
            assert_eq!(weights[0], 65536, "single weight must be exactly 1.0 fp16");
        }

        #[test]
        fn gaussian_weights_sum_to_one() {
            for radius in [1, 2, 4, 8, 16] {
                let sigma = radius as f32 / 3.0;
                let weights = vyre_primitives::math::conv1d::gaussian_weights(radius, sigma);
                let sum: u32 = weights.iter().sum();
                let diameter = weights.len() as u32;
                // Should sum to 65536 (1.0 in fp16.16) ±diameter for rounding.
                let lo = 65536u32.saturating_sub(diameter);
                let hi = 65536 + diameter;
                assert!(
                    (lo..=hi).contains(&sum),
                    "radius={radius}: weight sum {sum} not ≈65536 (tolerance ±{diameter})"
                );
            }
        }

        #[test]
        fn gaussian_weights_are_symmetric() {
            let weights = vyre_primitives::math::conv1d::gaussian_weights(4, 1.5);
            let n = weights.len();
            for i in 0..n / 2 {
                assert_eq!(
                    weights[i],
                    weights[n - 1 - i],
                    "weight {i} != weight {}",
                    n - 1 - i
                );
            }
        }

        #[test]
        fn program_builds_for_1x1() {
            // Edge case: 1×1 image should not panic.
            let stages = gaussian_blur_2pass("in", "out", "tmp", 1, 1, 2, 1.0);
            assert_eq!(stages.stage_count(), 2);
        }

        #[test]
        fn program_builds_for_max_radius() {
            let stages = gaussian_blur_2pass("in", "out", "tmp", 32, 32, 64, 20.0);
            assert_eq!(stages.stage_count(), 2);
        }

        #[test]
        fn reusable_kernel_builds_same_stage_shape_without_recomputing_weights() {
            let kernel = GaussianKernel::new(3, 1.25);
            let stages = gaussian_blur_2pass_with_kernel("in", "out", "tmp", 64, 32, &kernel);

            assert_eq!(kernel.radius(), 3);
            assert_eq!(kernel.weights().len(), 7);
            assert_eq!(stages.stage_count(), 2);
            assert_eq!(stages.horizontal.buffers()[0].name(), "in");
            assert_eq!(stages.horizontal.buffers()[1].name(), "tmp");
            assert_eq!(stages.vertical.buffers()[0].name(), "tmp");
            assert_eq!(stages.vertical.buffers()[1].name(), "out");
        }

        #[test]
        fn reusable_kernel_rejects_wrong_weight_count() {
            let err = GaussianKernel::from_weights(4, vec![65536; 3])
                .expect_err("radius 4 needs nine weights");

            assert_eq!(err.radius, 4);
            assert_eq!(err.expected, 9);
            assert_eq!(err.actual, 3);
            assert!(
                err.to_string().contains("Fix: supply 2 * radius + 1"),
                "kernel shape errors must be actionable"
            );
        }
    }

    // ================================================================
    // Shadow
    // ================================================================

    mod shadow {
        use vyre_libs::visual::shadow::box_shadow;

        #[test]
        fn program_has_correct_output_buffer() {
            let prog = box_shadow("output", 16, 16, 4, 4, 8, 8, 4.0, 0x80_000000);
            let bufs = prog.buffers();
            assert_eq!(bufs.len(), 1, "shadow uses only output buffer");
            assert_eq!(bufs[0].name(), "output");
            assert_eq!(prog.workgroup_size(), [256, 1, 1]);
        }

        #[test]
        fn builds_with_zero_rect() {
            // Degenerate case: zero-size rect should not panic.
            let prog = box_shadow("out", 8, 8, 0, 0, 0, 0, 1.0, 0xFF_FF0000);
            assert_eq!(prog.buffers().len(), 1);
        }

        #[test]
        fn builds_with_blur_zero() {
            // blur=0 is clamped to 1 internally.
            let prog = box_shadow("out", 8, 8, 2, 2, 4, 4, 0.0, 0xFF_000000);
            assert_eq!(prog.buffers().len(), 1);
        }
    }

    // ================================================================
    // Filter Chain
    // ================================================================

    mod filter_chain_tests {
        use vyre_libs::visual::filter_chain::filter_chain;

        #[test]
        fn identity_program_has_single_buffer() {
            let prog = filter_chain("pixels", 16, 1.0, 1.0, 1.0, 0.0);
            let bufs = prog.buffers();
            assert_eq!(bufs.len(), 1, "filter_chain works in-place");
            assert_eq!(bufs[0].name(), "pixels");
            assert_eq!(prog.workgroup_size(), [256, 1, 1]);
        }

        #[test]
        fn builds_with_zero_brightness() {
            let prog = filter_chain("px", 4, 0.0, 1.0, 1.0, 0.0);
            assert_eq!(prog.buffers().len(), 1);
        }

        #[test]
        fn builds_with_full_invert() {
            let prog = filter_chain("px", 4, 1.0, 1.0, 1.0, 1.0);
            assert_eq!(prog.buffers().len(), 1);
        }

        #[test]
        fn builds_with_extreme_contrast() {
            let prog = filter_chain("px", 4, 1.0, 3.0, 1.0, 0.0);
            assert_eq!(prog.buffers().len(), 1);
        }

        #[test]
        fn builds_with_desaturation() {
            let prog = filter_chain("px", 4, 1.0, 1.0, 0.0, 0.0);
            assert_eq!(prog.buffers().len(), 1);
        }
    }

    // ================================================================
    // Composite
    // ================================================================

    mod composite {
        use vyre_libs::visual::composite::alpha_over;

        #[test]
        fn program_has_three_buffers() {
            let prog = alpha_over("fg", "bg", "out", 16);
            let bufs = prog.buffers();
            assert_eq!(bufs.len(), 3, "composite: fg + bg + output");
            assert_eq!(prog.workgroup_size(), [256, 1, 1]);
        }

        #[test]
        fn builds_for_single_pixel() {
            let prog = alpha_over("f", "b", "o", 1);
            assert_eq!(prog.buffers().len(), 3);
        }
    }

    // ================================================================
    // Gradient
    // ================================================================

    mod gradient {
        use vyre_libs::visual::gradient::{linear_gradient, ColorStop};
        use vyre_reference::value::Value;

        fn render_u32(program: &vyre::ir::Program, pixels: usize) -> Vec<u32> {
            let init = vec![0u8; pixels * 4];
            let outputs = vyre_reference::reference_eval(
                program,
                &[Value::Bytes(std::sync::Arc::from(init.into_boxed_slice()))],
            )
            .expect("Fix: visual gradient program must execute in the reference interpreter.");
            outputs[0]
                .to_bytes()
                .chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                .collect()
        }

        #[test]
        fn builds_two_stop_horizontal() {
            let prog = linear_gradient(
                "out",
                16,
                1,
                90.0,
                &[
                    ColorStop {
                        position: 0.0,
                        color: 0xFF_0000FF,
                    },
                    ColorStop {
                        position: 1.0,
                        color: 0xFF_FF0000,
                    },
                ],
            );
            assert_eq!(prog.buffers().len(), 1);
            assert_eq!(prog.workgroup_size(), [256, 1, 1]);
        }

        #[test]
        fn builds_vertical_gradient() {
            let prog = linear_gradient(
                "out",
                1,
                16,
                0.0,
                &[
                    ColorStop {
                        position: 0.0,
                        color: 0xFF_000000,
                    },
                    ColorStop {
                        position: 1.0,
                        color: 0xFF_FFFFFF,
                    },
                ],
            );
            assert_eq!(prog.buffers().len(), 1);
        }

        #[test]
        fn css_angle_projection_matches_cardinal_directions() {
            let stops = [
                ColorStop {
                    position: 0.0,
                    color: 0xFF_0000FF,
                },
                ColorStop {
                    position: 1.0,
                    color: 0xFF_FF0000,
                },
            ];

            let horizontal = linear_gradient("out", 4, 1, 90.0, &stops);
            assert_eq!(
                render_u32(&horizontal, 4),
                [0xFF_0000FF, 0xFF_5500AA, 0xFF_AA0055, 0xFF_FF0000],
                "Fix: 90deg must project left-to-right."
            );

            let reverse_horizontal = linear_gradient("out", 4, 1, 270.0, &stops);
            assert_eq!(
                render_u32(&reverse_horizontal, 4),
                [0xFF_FF0000, 0xFF_AA0055, 0xFF_5500AA, 0xFF_0000FF],
                "Fix: 270deg must project right-to-left instead of clamping negative dots."
            );

            let upward = linear_gradient("out", 1, 4, 0.0, &stops);
            assert_eq!(
                render_u32(&upward, 4),
                [0xFF_FF0000, 0xFF_AA0055, 0xFF_5500AA, 0xFF_0000FF],
                "Fix: 0deg must project bottom-to-top under CSS angle semantics."
            );

            let downward = linear_gradient("out", 1, 4, 180.0, &stops);
            assert_eq!(
                render_u32(&downward, 4),
                [0xFF_0000FF, 0xFF_5500AA, 0xFF_AA0055, 0xFF_FF0000],
                "Fix: 180deg must project top-to-bottom."
            );
        }

        #[test]
        fn builds_multi_stop() {
            let prog = linear_gradient(
                "out",
                16,
                16,
                45.0,
                &[
                    ColorStop {
                        position: 0.0,
                        color: 0xFF_FF0000,
                    },
                    ColorStop {
                        position: 0.33,
                        color: 0xFF_00FF00,
                    },
                    ColorStop {
                        position: 0.66,
                        color: 0xFF_0000FF,
                    },
                    ColorStop {
                        position: 1.0,
                        color: 0xFF_FFFFFF,
                    },
                ],
            );
            assert_eq!(prog.buffers().len(), 1);
        }

        #[test]
        fn single_stop_returns_error_contract() {
            let error = vyre_libs::visual::gradient::try_linear_gradient(
                "out",
                4,
                4,
                0.0,
                &[ColorStop {
                    position: 0.0,
                    color: 0xFF_000000,
                }],
            )
            .expect_err("Fix: single-stop gradients must return an error contract");
            assert!(
                error.contains("2..=16 stops"),
                "Fix: gradient stop-count error must name the supported interval: {error}"
            );
        }
    }

    // ================================================================
    // Downsample
    // ================================================================

    mod downsample {
        use vyre_libs::visual::downsample::downsample_2x;

        #[test]
        fn program_has_correct_buffer_sizes() {
            let prog = downsample_2x("input", "output", 8, 8);
            let bufs = prog.buffers();
            assert_eq!(bufs.len(), 2);
            assert_eq!(bufs[0].name(), "input");
            assert_eq!(bufs[1].name(), "output");
            assert_eq!(prog.workgroup_size(), [256, 1, 1]);
        }

        #[test]
        fn builds_for_minimum_2x2() {
            let prog = downsample_2x("in", "out", 2, 2);
            assert_eq!(prog.buffers().len(), 2);
        }
    }

    // ================================================================
    // Glass (hero composition)
    // ================================================================

